@echo off
if /I "%1"=="exec" (
  shift
  codex exec --skip-git-repo-check %*
) else (
  codex %*
)
