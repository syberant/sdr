#!/usr/bin/env bash

_sd_completions() {
  local SD_ROOT="${SD_ROOT-"$HOME/sd"}"

  for W in "${COMP_WORDS[@]:1}"; do
    if [ ! -e "$SD_ROOT/$W" ]; then
      break
    fi

    SD_ROOT+="/$W"
  done

  if [ -f "$SD_ROOT" ]; then
    # We already found our file, no suggestions necessary
    COMPREPLY=()
  else
    COMPREPLY=($(ls -L "$SD_ROOT" | grep "^${COMP_WORDS[COMP_CWORD]}"))
  fi
}

complete -F _sd_completions sd
