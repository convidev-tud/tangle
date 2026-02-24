_tangl() {
  COMPREPLY=($(tangl __completion -i "${COMP_CWORD}" -- "${COMP_WORDS[@]}"))
}
complete -F _tangl tangl
