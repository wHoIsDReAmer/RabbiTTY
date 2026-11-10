if [[ -n ${RABBITTY_BASH_LOGIN-} ]]; then
  builtin unset RABBITTY_BASH_LOGIN
  if [[ -r /etc/profile ]]; then
    builtin source /etc/profile
  fi
  for _rabbitty_rc in ~/.bash_profile ~/.bash_login ~/.profile; do
    if [[ -r $_rabbitty_rc ]]; then
      builtin source "$_rabbitty_rc"
      break
    fi
  done
  builtin unset _rabbitty_rc
elif [[ -r ~/.bashrc ]]; then
  builtin source ~/.bashrc
fi
_rabbitty_prompt_start() {
  local ret=$?
  builtin printf '\e]133;D;%s\a\e]133;A\a' "$ret"
  return $ret
}
_rabbitty_prompt_end() {
  local ret=$?
  if [[ $PS1 != *'\e]133;B\a'* ]]; then
    PS1+='\[\e]133;B\a\]'
  fi
  return $ret
}
PS0+='\e]133;C\a'
PROMPT_COMMAND="_rabbitty_prompt_start${PROMPT_COMMAND:+; $PROMPT_COMMAND}; _rabbitty_prompt_end"
