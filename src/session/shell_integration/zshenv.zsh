if [[ -n ${RABBITTY_ZDOTDIR-} ]]; then
  ZDOTDIR=$RABBITTY_ZDOTDIR
else
  builtin unset ZDOTDIR
fi
builtin unset RABBITTY_ZDOTDIR
if [[ -r ${ZDOTDIR:-$HOME}/.zshenv ]]; then
  builtin source ${ZDOTDIR:-$HOME}/.zshenv
fi
if [[ -o interactive ]]; then
  _rabbitty_precmd() {
    builtin printf '\e]133;D;%s\a\e]133;A\a' $?
  }
  _rabbitty_preexec() {
    builtin printf '\e]133;C\a'
  }
  _rabbitty_line_init() {
    builtin printf '\e]133;B\a'
  }
  _rabbitty_install() {
    builtin emulate -L zsh
    precmd_functions=(_rabbitty_precmd ${precmd_functions:#(_rabbitty_install|_rabbitty_precmd)})
    preexec_functions=(${preexec_functions:#_rabbitty_preexec} _rabbitty_preexec)
    builtin zmodload zsh/zle
    builtin autoload -Uz add-zle-hook-widget
    add-zle-hook-widget line-init _rabbitty_line_init
    builtin printf '\e]133;A\a'
  }
  precmd_functions+=(_rabbitty_install)
fi
