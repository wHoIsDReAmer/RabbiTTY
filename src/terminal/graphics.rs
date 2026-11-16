use base64::Engine as _;
use base64::engine::DecodePaddingMode;
use base64::engine::general_purpose::{GeneralPurpose, GeneralPurposeConfig};
use std::collections::{HashMap, HashSet};
use std::io::{Cursor, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

pub const IMAGE_CELL: char = '\u{10EEEF}';

const MAX_CELLS: u16 = 4095;
const MAX_KEY: u32 = 0x00ff_ffff;
const MAX_DIMENSION: u32 = 8192;
const MAX_IMAGE_BYTES: usize = MAX_DIMENSION as usize * MAX_DIMENSION as usize * 4;
const QUOTA_BYTES: usize = 320 * 1024 * 1024;
const COLLECT_AT: usize = 256;
const TEMP_FILE_MARKER: &str = "tty-graphics-protocol";

const BASE64: GeneralPurpose = GeneralPurpose::new(
    &base64::alphabet::STANDARD,
    GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent),
);

static NEXT_UID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug)]
pub struct Image {
    pub uid: u64,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct ImageSlice {
    pub image: Arc<Image>,
    pub row: usize,
    pub col: usize,
    pub size: [f32; 2],
    pub uv: [f32; 4],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placed {
    pub key: u32,
    pub cols: u16,
    pub rows: u16,
    pub hold_cursor: bool,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Effects {
    pub place: Option<Placed>,
    pub clear_visible: bool,
    pub reply: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Command {
    action: u8,
    quiet: u8,
    format: u32,
    medium: u8,
    width: u32,
    height: u32,
    size: u32,
    offset: u32,
    id: u32,
    number: u32,
    placement: u32,
    compressed: bool,
    more: bool,
    source: [u32; 4],
    cols: u32,
    rows: u32,
    hold_cursor: bool,
    delete: u8,
    virtual_placement: bool,
}

impl Command {
    fn parse(control: &[u8]) -> Option<Self> {
        let mut command = Self {
            action: b't',
            quiet: 0,
            format: 32,
            medium: b'd',
            width: 0,
            height: 0,
            size: 0,
            offset: 0,
            id: 0,
            number: 0,
            placement: 0,
            compressed: false,
            more: false,
            source: [0; 4],
            cols: 0,
            rows: 0,
            hold_cursor: false,
            delete: b'a',
            virtual_placement: false,
        };
        for pair in control.split(|byte| *byte == b',') {
            let [key, b'=', value @ ..] = pair else {
                continue;
            };
            let letter = || value.first().copied();
            let number = || std::str::from_utf8(value).ok()?.parse::<u32>().ok();
            match key {
                b'a' => command.action = letter()?,
                b't' => command.medium = letter()?,
                b'd' => command.delete = letter()?,
                b'o' => command.compressed = value == b"z",
                b'q' => command.quiet = number()?.min(2) as u8,
                b'f' => command.format = number()?,
                b's' => command.width = number()?,
                b'v' => command.height = number()?,
                b'S' => command.size = number()?,
                b'O' => command.offset = number()?,
                b'i' => command.id = number()?,
                b'I' => command.number = number()?,
                b'p' => command.placement = number()?,
                b'm' => command.more = number()? == 1,
                b'x' => command.source[0] = number()?,
                b'y' => command.source[1] = number()?,
                b'w' => command.source[2] = number()?,
                b'h' => command.source[3] = number()?,
                b'c' => command.cols = number()?,
                b'r' => command.rows = number()?,
                b'C' => command.hold_cursor = number()? == 1,
                b'U' => command.virtual_placement = number()? == 1,
                _ => {}
            }
        }
        Some(command)
    }
}

#[derive(Debug)]
struct Transfer {
    command: Command,
    data: Vec<u8>,
}

#[derive(Debug)]
struct Placement {
    image: Arc<Image>,
    image_id: u32,
    placement_id: u32,
    source: [f32; 4],
    extent: [f32; 2],
    alt_screen: bool,
}

type Failure = &'static str;

#[derive(Debug)]
pub struct Graphics {
    transfer: Option<Transfer>,
    images: HashMap<u32, Arc<Image>>,
    numbers: HashMap<u32, u32>,
    placements: HashMap<u32, Placement>,
    next_key: u32,
    next_numbered_id: u32,
    collect_at: usize,
    quota: usize,
    file_access: bool,
}

impl Default for Graphics {
    fn default() -> Self {
        Self {
            transfer: None,
            images: HashMap::new(),
            numbers: HashMap::new(),
            placements: HashMap::new(),
            next_key: 1,
            next_numbered_id: u32::MAX,
            collect_at: COLLECT_AT,
            quota: QUOTA_BYTES,
            file_access: false,
        }
    }
}

impl Graphics {
    pub fn set_file_access(&mut self, allowed: bool) {
        self.file_access = allowed;
    }

    pub fn command(&mut self, payload: &[u8], cell: (u16, u16), alt_screen: bool) -> Effects {
        let split = payload
            .iter()
            .position(|byte| *byte == b';')
            .unwrap_or(payload.len());
        let (control, encoded) = (&payload[..split], payload.get(split + 1..).unwrap_or(&[]));
        let pending = self.transfer.take();
        let Some(parsed) = Command::parse(control) else {
            return Effects::default();
        };
        let first = pending.as_ref().map_or(parsed, |transfer| transfer.command);
        let Ok(decoded) = BASE64.decode(encoded) else {
            return Self::failed(&first, "EINVAL:the payload is not base64");
        };

        let (command, data) = match pending {
            Some(mut transfer) => {
                if transfer.data.len() + decoded.len() > MAX_IMAGE_BYTES {
                    return Self::failed(&first, "EFBIG:the image data is too large");
                }
                transfer.data.extend_from_slice(&decoded);
                if parsed.more {
                    self.transfer = Some(transfer);
                    return Effects::default();
                }
                (transfer.command, transfer.data)
            }
            None if parsed.more => {
                self.transfer = Some(Transfer {
                    command: parsed,
                    data: decoded,
                });
                return Effects::default();
            }
            None => (parsed, decoded),
        };

        match self.run(&command, data, cell, alt_screen) {
            Ok(effects) => effects,
            Err(failure) => Self::failed(&command, failure),
        }
    }

    fn run(
        &mut self,
        command: &Command,
        data: Vec<u8>,
        cell: (u16, u16),
        alt_screen: bool,
    ) -> Result<Effects, Failure> {
        let unsupported = "ENOTSUPPORTED:unicode placeholders are not supported";
        match command.action {
            b'q' => {
                self.load(command, data)?;
                Ok(Effects {
                    reply: Self::reply(command, command.id, Ok(())),
                    ..Effects::default()
                })
            }
            b't' => {
                let image = Arc::new(self.load(command, data)?);
                let id = self.remember(command, &image);
                Ok(Effects {
                    reply: Self::reply(command, id, Ok(())),
                    ..Effects::default()
                })
            }
            b'T' if command.virtual_placement => Err(unsupported),
            b'T' => {
                let image = Arc::new(self.load(command, data)?);
                let id = self.remember(command, &image);
                let placed = self.place(image, id, command, cell, alt_screen)?;
                Ok(Effects {
                    place: Some(placed),
                    reply: Self::reply(command, id, Ok(())),
                    ..Effects::default()
                })
            }
            b'p' if command.virtual_placement => Err(unsupported),
            b'p' => {
                let (id, image) = self.find(command).ok_or("ENOENT:no such image")?;
                let placed = self.place(image, id, command, cell, alt_screen)?;
                Ok(Effects {
                    place: Some(placed),
                    reply: Self::reply(command, id, Ok(())),
                    ..Effects::default()
                })
            }
            b'd' => Ok(Effects {
                clear_visible: self.delete(command),
                ..Effects::default()
            }),
            _ => Err("EINVAL:unsupported action"),
        }
    }

    fn failed(command: &Command, failure: Failure) -> Effects {
        Effects {
            reply: Self::reply(command, command.id, Err(failure)),
            ..Effects::default()
        }
    }

    fn reply(command: &Command, id: u32, result: Result<(), Failure>) -> Option<String> {
        if command.id == 0 && command.number == 0 {
            return None;
        }
        let silenced = match result {
            Ok(()) => command.quiet >= 1,
            Err(_) => command.quiet >= 2,
        };
        if silenced {
            return None;
        }
        let mut keys = format!("i={id}");
        if command.number != 0 {
            keys.push_str(&format!(",I={}", command.number));
        }
        if command.placement != 0 {
            keys.push_str(&format!(",p={}", command.placement));
        }
        let message = result.err().unwrap_or("OK");
        Some(format!("\x1b_G{keys};{message}\x1b\\"))
    }

    fn find(&self, command: &Command) -> Option<(u32, Arc<Image>)> {
        let id = if command.id != 0 {
            command.id
        } else {
            *self.numbers.get(&command.number)?
        };
        Some((id, Arc::clone(self.images.get(&id)?)))
    }

    fn remember(&mut self, command: &Command, image: &Arc<Image>) -> u32 {
        let id = match (command.id, command.number) {
            (0, 0) => return 0,
            (0, number) => {
                let id = self.next_numbered_id;
                self.next_numbered_id = if id <= 1 { u32::MAX } else { id - 1 };
                self.numbers.insert(number, id);
                id
            }
            (id, _) => id,
        };
        self.placements
            .retain(|_, placement| placement.image_id != id);
        self.images.insert(id, Arc::clone(image));
        id
    }

    fn place(
        &mut self,
        image: Arc<Image>,
        id: u32,
        command: &Command,
        cell: (u16, u16),
        alt_screen: bool,
    ) -> Result<Placed, Failure> {
        let [x, y, w, h] = command.source;
        if x >= image.width || y >= image.height {
            return Err("EINVAL:the source rectangle is outside the image");
        }
        let w = if w == 0 { image.width - x } else { w }.min(image.width - x);
        let h = if h == 0 { image.height - y } else { h }.min(image.height - y);
        let cell = [f32::from(cell.0.max(1)), f32::from(cell.1.max(1))];
        let natural = [w as f32 / cell[0], h as f32 / cell[1]];
        let limit = f32::from(MAX_CELLS);
        let extent = match (command.cols, command.rows) {
            (0, 0) => natural,
            (cols, 0) => [cols as f32, natural[1] * cols as f32 / natural[0]],
            (0, rows) => [natural[0] * rows as f32 / natural[1], rows as f32],
            (cols, rows) => [cols as f32, rows as f32],
        };
        let extent = [extent[0].min(limit), extent[1].min(limit)];
        let cells = |length: f32| (length.ceil() as u16).clamp(1, MAX_CELLS);

        let key = self.fresh_key();
        self.placements.insert(
            key,
            Placement {
                image,
                image_id: id,
                placement_id: command.placement,
                source: [x as f32, y as f32, w as f32, h as f32],
                extent,
                alt_screen,
            },
        );
        Ok(Placed {
            key,
            cols: cells(extent[0]),
            rows: cells(extent[1]),
            hold_cursor: command.hold_cursor,
        })
    }

    fn fresh_key(&mut self) -> u32 {
        loop {
            let key = self.next_key;
            self.next_key = if key >= MAX_KEY { 1 } else { key + 1 };
            if !self.placements.contains_key(&key) {
                return key;
            }
        }
    }

    fn delete(&mut self, command: &Command) -> bool {
        let id = match command.delete {
            b'a' | b'A' => return true,
            b'i' | b'I' => command.id,
            b'n' | b'N' => self.numbers.get(&command.number).copied().unwrap_or(0),
            _ => 0,
        };
        if id == 0 {
            return false;
        }
        self.placements.retain(|_, placement| {
            placement.image_id != id
                || (command.placement != 0 && placement.placement_id != command.placement)
        });
        if command.delete.is_ascii_uppercase() {
            self.images.remove(&id);
            self.numbers.retain(|_, numbered| *numbered != id);
        }
        false
    }

    pub fn slice(
        &self,
        key: u32,
        image_row: u16,
        first_col: u16,
        len: u16,
    ) -> Option<(Arc<Image>, [f32; 2], [f32; 4])> {
        let placement = self.placements.get(&key)?;
        let [width, height] = placement.extent;
        let left = f32::from(first_col);
        let right = (left + f32::from(len)).min(width);
        let top = f32::from(image_row);
        let bottom = (top + 1.0).min(height);
        if right <= left || bottom <= top {
            return None;
        }
        let [x, y, w, h] = placement.source;
        let image = &placement.image;
        let u = |cells: f32| (x + cells / width * w) / image.width as f32;
        let v = |cells: f32| (y + cells / height * h) / image.height as f32;
        Some((
            Arc::clone(image),
            [right - left, bottom - top],
            [u(left), v(top), u(right), v(bottom)],
        ))
    }

    pub fn is_idle(&self) -> bool {
        self.placements.is_empty()
    }

    #[cfg(test)]
    pub(crate) fn placements(&self) -> usize {
        self.placements.len()
    }

    pub fn over_budget(&self) -> bool {
        self.placements.len() >= self.collect_at || self.held_bytes() > self.quota
    }

    pub fn collect(&mut self, alt_screen: bool, live: &HashSet<u32>) {
        self.placements.retain(|key, placement| {
            if placement.alt_screen == alt_screen {
                live.contains(key)
            } else {
                !placement.alt_screen
            }
        });
        while self.held_bytes() > self.quota {
            let placed: HashSet<u64> = self
                .placements
                .values()
                .map(|placement| placement.image.uid)
                .collect();
            let unplaced = self
                .images
                .iter()
                .filter(|(_, image)| !placed.contains(&image.uid))
                .min_by_key(|(_, image)| image.uid)
                .map(|(id, _)| *id);
            if let Some(id) = unplaced {
                self.images.remove(&id);
                continue;
            }
            let oldest = self
                .placements
                .iter()
                .min_by_key(|(_, placement)| placement.image.uid)
                .map(|(_, placement)| placement.image.uid);
            let Some(uid) = oldest else {
                break;
            };
            self.placements
                .retain(|_, placement| placement.image.uid != uid);
            self.images.retain(|_, image| image.uid != uid);
        }
        self.collect_at = COLLECT_AT.max(self.placements.len() * 2);
    }

    fn held_bytes(&self) -> usize {
        let mut seen = HashSet::new();
        self.images
            .values()
            .chain(self.placements.values().map(|placement| &placement.image))
            .filter(|image| seen.insert(image.uid))
            .map(|image| image.rgba.len())
            .sum()
    }

    fn load(&self, command: &Command, data: Vec<u8>) -> Result<Image, Failure> {
        let raw = match command.medium {
            b'd' => data,
            b'f' | b't' => self.read_file(command, &data)?,
            _ => return Err("ENOTSUPPORTED:this transmission medium is not supported"),
        };
        let raw = if command.compressed {
            miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&raw, MAX_IMAGE_BYTES)
                .map_err(|_| "EINVAL:the data could not be decompressed")?
        } else {
            raw
        };
        let (width, height, rgba) = match command.format {
            100 => decode_png(&raw)?,
            24 | 32 => {
                let (width, height) = (command.width, command.height);
                if width == 0 || height == 0 {
                    return Err("EINVAL:the image has no size");
                }
                if width > MAX_DIMENSION || height > MAX_DIMENSION {
                    return Err("EFBIG:the image is too large");
                }
                let pixels = width as usize * height as usize;
                let channels = if command.format == 24 { 3 } else { 4 };
                if raw.len() < pixels * channels {
                    return Err("ENODATA:the image data is shorter than its size");
                }
                let rgba = if channels == 4 {
                    let mut raw = raw;
                    raw.truncate(pixels * 4);
                    raw
                } else {
                    raw[..pixels * 3]
                        .chunks_exact(3)
                        .flat_map(|pixel| [pixel[0], pixel[1], pixel[2], 255])
                        .collect()
                };
                (width, height, rgba)
            }
            _ => return Err("EINVAL:unknown image format"),
        };
        Ok(Image {
            uid: NEXT_UID.fetch_add(1, Ordering::Relaxed),
            width,
            height,
            rgba,
        })
    }

    fn read_file(&self, command: &Command, encoded_path: &[u8]) -> Result<Vec<u8>, Failure> {
        if !self.file_access {
            return Err("EPERM:files cannot be read from this session");
        }
        let path = std::str::from_utf8(encoded_path)
            .map(PathBuf::from)
            .map_err(|_| "EINVAL:the file path is not valid")?;
        let temporary = command.medium == b't';
        if temporary && !is_graphics_temp_file(&path) {
            return Err("EPERM:not a graphics temporary file");
        }
        let unreadable = "EBADF:the file could not be read";
        let metadata = std::fs::metadata(&path).map_err(|_| unreadable)?;
        if !metadata.is_file() {
            return Err("EBADF:not a regular file");
        }
        let available = metadata.len().saturating_sub(u64::from(command.offset));
        let wanted = if command.size == 0 {
            available
        } else {
            available.min(u64::from(command.size))
        };
        if wanted > MAX_IMAGE_BYTES as u64 {
            return Err("EFBIG:the file is too large");
        }
        let mut file = std::fs::File::open(&path).map_err(|_| unreadable)?;
        file.seek(SeekFrom::Start(u64::from(command.offset)))
            .map_err(|_| unreadable)?;
        let mut data = Vec::with_capacity(wanted as usize);
        file.take(wanted)
            .read_to_end(&mut data)
            .map_err(|_| unreadable)?;
        if temporary {
            let _ = std::fs::remove_file(&path);
        }
        Ok(data)
    }
}

fn decode_png(raw: &[u8]) -> Result<(u32, u32, Vec<u8>), Failure> {
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_DIMENSION);
    limits.max_image_height = Some(MAX_DIMENSION);
    let mut reader = image::ImageReader::with_format(Cursor::new(raw), image::ImageFormat::Png);
    reader.limits(limits);
    let rgba = reader
        .decode()
        .map_err(|_| "EINVAL:the data is not a PNG image this terminal can show")?
        .into_rgba8();
    Ok((rgba.width(), rgba.height(), rgba.into_raw()))
}

fn is_graphics_temp_file(path: &Path) -> bool {
    let named = path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.contains(TEMP_FILE_MARKER));
    let Ok(resolved) = std::fs::canonicalize(path) else {
        return false;
    };
    named
        && [std::env::temp_dir(), "/tmp".into(), "/dev/shm".into()]
            .iter()
            .filter_map(|dir| std::fs::canonicalize(dir).ok())
            .any(|dir| resolved.starts_with(dir))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    const CELL: (u16, u16) = (10, 20);

    pub(crate) fn payload(control: &str, data: &[u8]) -> Vec<u8> {
        use base64::engine::general_purpose::STANDARD;
        format!("{control};{}", STANDARD.encode(data)).into_bytes()
    }

    pub(crate) fn png(width: u32, height: u32, rgba: [u8; 4]) -> Vec<u8> {
        let pixels: Vec<u8> = rgba
            .iter()
            .copied()
            .cycle()
            .take((width * height * 4) as usize)
            .collect();
        let image = image::RgbaImage::from_raw(width, height, pixels).expect("pixels fit");
        let mut encoded = Cursor::new(Vec::new());
        image
            .write_to(&mut encoded, image::ImageFormat::Png)
            .expect("png encodes");
        encoded.into_inner()
    }

    fn show(graphics: &mut Graphics, control: &str, data: &[u8]) -> Effects {
        graphics.command(&payload(control, data), CELL, false)
    }

    fn placed(effects: &Effects) -> Placed {
        effects.place.expect("the command should place an image")
    }

    #[test]
    fn an_image_takes_as_many_cells_as_its_pixels_cover() {
        let mut graphics = Graphics::default();

        let shown = show(&mut graphics, "a=T,f=100", &png(25, 41, [255, 0, 0, 255]));

        let placed = placed(&shown);
        assert_eq!((placed.cols, placed.rows), (3, 3));
        assert_eq!(shown.reply, None, "a command without an id gets no answer");
        let (image, size, uv) = graphics.slice(placed.key, 2, 0, 3).expect("last row");
        assert_eq!((image.width, image.height), (25, 41));
        assert!((size[0] - 2.5).abs() < 1e-4 && (size[1] - 0.05).abs() < 1e-4);
        assert!((uv[1] - 40.0 / 41.0).abs() < 1e-4 && (uv[3] - 1.0).abs() < 1e-4);
    }

    #[test]
    fn an_image_sent_in_chunks_is_shown_once_the_last_chunk_arrives() {
        let mut graphics = Graphics::default();
        let encoded = payload("", &png(4, 4, [0, 255, 0, 255]));
        let encoded = &encoded[1..];
        let (head, tail) = encoded.split_at(encoded.len() / 2 / 4 * 4);

        let first = [b"a=T,f=100,i=7,m=1;", head].concat();
        let last = [b"m=0;", tail].concat();

        assert_eq!(graphics.command(&first, CELL, false), Effects::default());
        let shown = graphics.command(&last, CELL, false);
        assert_eq!((placed(&shown).cols, placed(&shown).rows), (1, 1));
        assert_eq!(shown.reply.as_deref(), Some("\x1b_Gi=7;OK\x1b\\"));
    }

    #[test]
    fn raw_pixels_are_accepted_with_or_without_alpha_and_compression() {
        let mut graphics = Graphics::default();
        let rgb = [9, 8, 7, 6, 5, 4];
        let packed = miniz_oxide::deflate::compress_to_vec_zlib(&rgb, 6);

        let plain = show(&mut graphics, "a=T,f=24,s=2,v=1", &rgb);
        let compressed = show(&mut graphics, "a=T,f=24,s=2,v=1,o=z", &packed);
        let short = show(&mut graphics, "a=T,f=32,s=2,v=1,i=3", &rgb);

        for shown in [&plain, &compressed] {
            let (image, _, _) = graphics.slice(placed(shown).key, 0, 0, 1).expect("slice");
            assert_eq!(image.rgba, [9, 8, 7, 255, 6, 5, 4, 255]);
        }
        assert_eq!(short.place, None);
        assert!(
            short.reply.is_some_and(|reply| reply.contains("ENODATA")),
            "too little data for the declared size must be refused"
        );
    }

    #[test]
    fn a_query_is_answered_without_keeping_or_showing_the_image() {
        let mut graphics = Graphics::default();

        let supported = show(&mut graphics, "a=q,f=24,s=1,v=1,i=1", &[1, 2, 3]);
        let shared_memory = show(&mut graphics, "a=q,f=24,t=s,s=1,v=1,i=3", b"/name");
        let later = show(&mut graphics, "a=p,i=1", &[]);

        assert_eq!(supported.reply.as_deref(), Some("\x1b_Gi=1;OK\x1b\\"));
        assert_eq!(supported.place, None);
        assert!(
            shared_memory
                .reply
                .is_some_and(|reply| reply.starts_with("\x1b_Gi=3;ENOTSUPPORTED"))
        );
        assert!(later.reply.is_some_and(|reply| reply.contains("ENOENT")));
    }

    #[test]
    fn quiet_levels_silence_success_and_then_errors() {
        let mut graphics = Graphics::default();

        let ok_quiet = show(&mut graphics, "a=q,f=24,s=1,v=1,i=1,q=1", &[1, 2, 3]);
        let error_at_1 = show(&mut graphics, "a=q,f=24,s=9,v=9,i=1,q=1", &[1, 2, 3]);
        let error_at_2 = show(&mut graphics, "a=q,f=24,s=9,v=9,i=1,q=2", &[1, 2, 3]);

        assert_eq!(ok_quiet.reply, None);
        assert!(error_at_1.reply.is_some());
        assert_eq!(error_at_2.reply, None);
    }

    #[test]
    fn a_stored_image_can_be_shown_scaled_and_cropped() {
        let mut graphics = Graphics::default();
        show(
            &mut graphics,
            "a=t,f=100,i=5",
            &png(40, 40, [0, 0, 255, 255]),
        );

        let wide = placed(&show(&mut graphics, "a=p,i=5,c=8", &[]));
        let cropped = placed(&show(
            &mut graphics,
            "a=p,i=5,x=20,y=10,w=10,h=20,c=2,r=2",
            &[],
        ));

        assert_eq!(
            (wide.cols, wide.rows),
            (8, 4),
            "rows follow the aspect ratio"
        );
        let (_, _, uv) = graphics
            .slice(cropped.key, 1, 1, 1)
            .expect("bottom right cell");
        assert_eq!(uv, [0.625, 0.5, 0.75, 0.75]);
    }

    #[test]
    fn deleting_an_image_by_id_stops_it_from_being_drawn() {
        let mut graphics = Graphics::default();
        let kept = placed(&show(
            &mut graphics,
            "a=T,f=100,i=1",
            &png(10, 20, [1, 1, 1, 255]),
        ));
        let gone = placed(&show(
            &mut graphics,
            "a=T,f=100,i=2",
            &png(10, 20, [2, 2, 2, 255]),
        ));

        let by_id = show(&mut graphics, "a=d,d=i,i=2", &[]);
        let everything = show(&mut graphics, "a=d", &[]);

        assert!(!by_id.clear_visible);
        assert!(graphics.slice(gone.key, 0, 0, 1).is_none());
        assert!(graphics.slice(kept.key, 0, 0, 1).is_some());
        assert!(everything.clear_visible);
        assert!(
            show(&mut graphics, "a=p,i=2", &[]).place.is_some(),
            "a lowercase delete keeps the image data"
        );
    }

    #[test]
    fn sending_an_id_again_replaces_the_image_and_drops_where_it_was_shown() {
        let mut graphics = Graphics::default();
        let old = placed(&show(
            &mut graphics,
            "a=T,f=100,i=1",
            &png(10, 20, [1, 1, 1, 255]),
        ));

        show(&mut graphics, "a=t,f=100,i=1", &png(30, 20, [2, 2, 2, 255]));

        assert!(graphics.slice(old.key, 0, 0, 1).is_none());
        assert_eq!(placed(&show(&mut graphics, "a=p,i=1", &[])).cols, 3);
    }

    #[test]
    fn files_are_read_only_where_the_session_allows_it() {
        let dir = std::env::temp_dir().join(format!("rabbitty-graphics-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        let ordinary = dir.join("picture.png");
        let temporary = dir.join("x-tty-graphics-protocol-1.rgb");
        std::fs::write(&ordinary, png(10, 20, [3, 3, 3, 255])).expect("file");
        std::fs::write(&temporary, [1, 2, 3]).expect("file");
        let path = |file: &Path| file.to_string_lossy().into_owned().into_bytes();

        let mut remote = Graphics::default();
        let refused = show(&mut remote, "a=T,f=100,t=f,i=1", &path(&ordinary));
        assert!(refused.reply.is_some_and(|reply| reply.contains("EPERM")));
        assert_eq!(refused.place, None);

        let mut local = Graphics::default();
        local.set_file_access(true);
        assert!(
            show(&mut local, "a=T,f=100,t=f", &path(&ordinary))
                .place
                .is_some()
        );
        let not_temporary = show(&mut local, "a=T,f=100,t=t,i=2", &path(&ordinary));
        assert!(
            not_temporary
                .reply
                .is_some_and(|reply| reply.contains("EPERM"))
        );
        assert!(
            ordinary.exists(),
            "only marked temporary files may be deleted"
        );
        let oversized = show(
            &mut local,
            "a=q,f=24,t=t,s=1,v=1,S=99,i=3",
            &path(&temporary),
        );
        assert_eq!(oversized.reply.as_deref(), Some("\x1b_Gi=3;OK\x1b\\"));
        assert!(!temporary.exists(), "a temporary file is removed once read");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn collecting_keeps_what_is_on_screen_and_the_main_screen_behind_a_fullscreen_program() {
        let mut graphics = Graphics::default();
        let picture = png(10, 20, [4, 4, 4, 255]);
        let on_main = placed(&show(&mut graphics, "a=T,f=100", &picture));
        let erased = placed(&show(&mut graphics, "a=T,f=100", &picture));
        let on_alt = placed(&graphics.command(&payload("a=T,f=100", &picture), CELL, true));

        graphics.collect(true, &HashSet::from([on_alt.key]));
        assert!(graphics.slice(on_main.key, 0, 0, 1).is_some());
        assert!(graphics.slice(erased.key, 0, 0, 1).is_some());

        graphics.collect(false, &HashSet::from([on_main.key]));
        assert!(graphics.slice(on_main.key, 0, 0, 1).is_some());
        assert!(graphics.slice(erased.key, 0, 0, 1).is_none());
        assert!(graphics.slice(on_alt.key, 0, 0, 1).is_none());
    }

    #[test]
    fn going_over_the_memory_quota_evicts_unshown_images_before_shown_ones() {
        let mut graphics = Graphics {
            quota: 10 * 20 * 4 * 2,
            ..Graphics::default()
        };
        let picture = png(10, 20, [5, 5, 5, 255]);
        let first = placed(&show(&mut graphics, "a=T,f=100,i=1", &picture));
        show(&mut graphics, "a=t,f=100,i=2", &picture);
        let third = placed(&show(&mut graphics, "a=T,f=100,i=3", &picture));
        assert!(graphics.over_budget());

        graphics.collect(false, &HashSet::from([first.key, third.key]));

        assert!(!graphics.over_budget());
        assert!(graphics.slice(first.key, 0, 0, 1).is_some());
        assert!(graphics.slice(third.key, 0, 0, 1).is_some());
        assert!(show(&mut graphics, "a=p,i=2", &[]).place.is_none());

        let fourth = placed(&show(&mut graphics, "a=T,f=100,i=4", &picture));
        graphics.collect(false, &HashSet::from([first.key, third.key, fourth.key]));
        assert!(
            graphics.slice(first.key, 0, 0, 1).is_none(),
            "the oldest goes first"
        );
        assert!(graphics.slice(fourth.key, 0, 0, 1).is_some());
    }
}
