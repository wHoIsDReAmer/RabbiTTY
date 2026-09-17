#[allow(clippy::all)]
pub mod bindings {
    wit_bindgen::generate!({
        path: "../../wit",
        world: "plugin",
        pub_export_macro: true,
        default_bindings_module: "rabbitty_plugin_sdk::bindings",
        additional_derives: [PartialEq],
    });
}

pub mod http;

pub use bindings::rabbitty::plugin::host::read_config;
pub use bindings::rabbitty::plugin::types::*;

pub trait Plugin {
    fn manifest() -> PluginInfo;

    fn contributions() -> Contributions;

    fn init(&mut self) -> Result<Vec<Action>, String> {
        Ok(Vec::new())
    }

    fn shutdown(&mut self) -> Vec<Action> {
        Vec::new()
    }

    fn on_event(&mut self, event: Event) -> Vec<Action> {
        let _ = event;
        Vec::new()
    }

    fn run_command(&mut self, id: &str) -> Result<Vec<Action>, String> {
        let _ = id;
        Ok(Vec::new())
    }

    fn list_profiles(&mut self) -> Result<Vec<PluginProfile>, String> {
        Ok(Vec::new())
    }
}

#[macro_export]
macro_rules! export_plugin {
    ($ty:ty) => {
        const _: () = {
            ::std::thread_local! {
                static INSTANCE: ::std::cell::RefCell<::core::option::Option<$ty>> =
                    const { ::std::cell::RefCell::new(::core::option::Option::None) };
            }

            fn with_instance<R>(f: impl FnOnce(&mut $ty) -> R) -> R {
                INSTANCE.with(|slot| {
                    f(slot
                        .borrow_mut()
                        .get_or_insert_with(<$ty as ::core::default::Default>::default))
                })
            }

            struct RabbittyPluginExport;

            impl $crate::bindings::Guest for RabbittyPluginExport {
                fn manifest() -> $crate::PluginInfo {
                    <$ty as $crate::Plugin>::manifest()
                }

                fn init()
                -> ::core::result::Result<::std::vec::Vec<$crate::Action>, ::std::string::String>
                {
                    INSTANCE.with(|slot| {
                        *slot.borrow_mut() = ::core::option::Option::Some(
                            <$ty as ::core::default::Default>::default(),
                        );
                    });
                    with_instance(<$ty as $crate::Plugin>::init)
                }

                fn shutdown()
                -> ::core::result::Result<::std::vec::Vec<$crate::Action>, ::std::string::String>
                {
                    ::core::result::Result::Ok(with_instance(<$ty as $crate::Plugin>::shutdown))
                }

                fn contributions()
                -> ::core::result::Result<$crate::Contributions, ::std::string::String> {
                    ::core::result::Result::Ok(<$ty as $crate::Plugin>::contributions())
                }

                fn on_event(
                    ev: $crate::Event,
                ) -> ::core::result::Result<::std::vec::Vec<$crate::Action>, ::std::string::String>
                {
                    ::core::result::Result::Ok(with_instance(|plugin| {
                        <$ty as $crate::Plugin>::on_event(plugin, ev)
                    }))
                }

                fn run_command(
                    id: ::std::string::String,
                ) -> ::core::result::Result<::std::vec::Vec<$crate::Action>, ::std::string::String>
                {
                    with_instance(|plugin| <$ty as $crate::Plugin>::run_command(plugin, &id))
                }

                fn list_profiles() -> ::core::result::Result<
                    ::std::vec::Vec<$crate::PluginProfile>,
                    ::std::string::String,
                > {
                    with_instance(<$ty as $crate::Plugin>::list_profiles)
                }
            }

    $crate::bindings::export!(RabbittyPluginExport with_types_in $crate::bindings);
        };
    };
}
