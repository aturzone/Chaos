@echo off
setlocal
rem Run Claude Code against a local Chaos node instead of Anthropic's API.
rem
rem Modelled on Atur's own `claude-free`, which proves the mechanism: Claude
rem Code reads ANTHROPIC_BASE_URL and will talk to whatever answers there. What
rem it needs from the other end is the Messages API with tools, which Chaos
rem serves since v0.0.33 -- see `network/serve/src/anthropic.rs`.
rem
rem   claude-chaos.cmd "your prompt"        one turn, then exit
rem   claude-chaos.cmd                      interactive
rem
rem Everything here is a variable so it can be overridden from the caller:
rem   set CHAOS_PORT=8231 & claude-chaos.cmd ...

if "%CHAOS_PORT%"=="" set "CHAOS_PORT=8231"
if "%CHAOS_KEY%"=="" set "CHAOS_KEY=local"

rem **The tool set is the single biggest lever on whether this works at all.**
rem Claude Code's default twenty-eight tool definitions are 40,255 tokens --
rem more than the 32,768-token context of every model that runs here, before
rem the user types anything. Six brings it to 11,706 and three to 10,564.
rem Measured: research/claude-code-against-a-chaos-node-2026-09-07.md
if "%CHAOS_TOOLS%"=="" set "CHAOS_TOOLS=Read,Write,Edit,Bash,Glob,Grep"

rem **A model name from Claude Code's own catalogue, not the local model's.**
rem The name is validated client-side BEFORE any request is sent: an unknown
rem one gives `[claude-code:unrecognized_model]` and exit 0, with the node never
rem seeing a byte. The node ignores the name and answers with whatever it has
rem loaded, which is what `claude-free` does too.
if "%CHAOS_MODEL%"=="" set "CHAOS_MODEL=claude-opus-5"

rem The real window of the local model. Without this Claude Code assumes 200k
rem and compacts against a number the node cannot hold.
if "%CHAOS_CONTEXT%"=="" set "CHAOS_CONTEXT=16384"

rem Its own config directory, so pointing Claude Code at a local model does not
rem disturb the history, settings or authentication of a normal `claude`.
set "CLAUDE_CONFIG_DIR=%USERPROFILE%\.claude-chaos"

where claude >nul 2>&1
if errorlevel 1 (
  echo Claude Code is not installed, or not on PATH.
  echo.
  echo   npm install -g @anthropic-ai/claude-code
  echo.
  echo That needs Node.js from https://nodejs.org. Then run this again.
  exit /b 127
)

curl -s -m 3 -o nul "http://127.0.0.1:%CHAOS_PORT%/health"
if errorlevel 1 (
  echo No Chaos node is answering on port %CHAOS_PORT%.
  echo.
  echo Start one first, with a model that can hold a conversation:
  echo.
  echo   chaos-serve ^<model.gguf^> --port %CHAOS_PORT% --context %CHAOS_CONTEXT%
  echo.
  echo The model to use is the one that will call tools, which is not the
  echo best one at code -- Qwen2.5-Coder-7B would not, Qwen3-4B does:
  echo   C:\Projects\models\qwen3-4b\Qwen3-4B-Q4_K_M.gguf
  echo.
  echo Then run this again. `chaos start` also works if the node is configured.
  exit /b 1
)

set "ANTHROPIC_BASE_URL=http://127.0.0.1:%CHAOS_PORT%"
set "ANTHROPIC_API_KEY=%CHAOS_KEY%"
set "ANTHROPIC_MODEL=%CHAOS_MODEL%"
set "CLAUDE_CODE_MAX_CONTEXT_TOKENS=%CHAOS_CONTEXT%"

echo Claude Code -^> Chaos on port %CHAOS_PORT%, tools: %CHAOS_TOOLS%
echo A turn takes minutes on a CPU machine. That is the model, not a hang.
echo.
claude --tools "%CHAOS_TOOLS%" %*
