@echo off
rem rust-checks-windows.cmd — OneDev's "rust checks windows": clippy and the tests of apps/k7/backend-rs and the browser crates (fmt too) on the Windows agent.
rem Each ci/ branch keeps its target folder in %LOCALAPPDATA%\k7-ci\rust; one not checked for 2 days is removed (docs/release.md).
rem An area the branch did not touch since base/main is skipped (engine\scripts\ci\areas.mjs, docs/rust-builds.md).
setlocal
where cargo > nul 2>&1 || if exist "%USERPROFILE%\.cargo\bin\cargo.exe" set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
rem cc-rs finds MSVC through vswhere, but the developer environment makes the linker and cl.exe certain.
for /f "usebackq delims=" %%V in (`"%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe" -latest -products * -property installationPath`) do set "VSDIR=%%V"
call "%VSDIR%\VC\Auxiliary\Build\vcvars64.bat" > nul || exit /b 1
set "SLOTS=%LOCALAPPDATA%\k7-ci\rust"
for /f "usebackq delims=" %%S in (`node -e "console.log(process.argv[1].replace(/[^A-Za-z0-9._-]/g,'_'))" "%~1"`) do set "SLOT=%SLOTS%\%%S"
if not exist "%SLOT%" mkdir "%SLOT%" || exit /b 1
rem The agent is also the developers' machine: old folders go after 2 days, and the build takes a third of its cores.
node -e "const fs=require('fs'),p=require('path'),d=process.argv[1],now=Date.now();for(const n of fs.readdirSync(d)){const f=p.join(d,n);if(now-fs.statSync(f).mtimeMs>2*864e5)fs.rmSync(f,{recursive:true,force:true})}" "%SLOTS%" || exit /b 1
copy /b "%SLOT%\.touch" +,, > nul 2>&1 || type nul > "%SLOT%\.touch"
call npm ci --ignore-scripts --no-audit --no-fund || exit /b 1
set "AREAS="
for /f "usebackq delims=" %%A in (`node engine\scripts\ci\areas.mjs`) do set "AREAS=%%A"
echo areas touched since base/main: [%AREAS%]
rem rustc through sccache when the agent has one, on the checks' own server and folder (docs/rust-builds.md).
for /f "usebackq delims=" %%L in (`node engine\scripts\sccache.mjs checks`) do set "%%L"
rem --target-dir, not CARGO_TARGET_DIR: sccache keys on every CARGO_ variable, and the folder is per branch.
set "TARGET=%SLOT%\target"
set "CARGO_PROFILE_DEV_DEBUG=0"
rem No incremental build: its caches are most of a target folder's writes, and sccache caches no crate built that way.
set "CARGO_INCREMENTAL=0"
set "CARGO_BUILD_JOBS=4"
set "NEXTEST_TEST_THREADS=4"
set "ROOT=%CD%"
echo [%AREAS%] | findstr /c:"backend" > nul || goto browserpart
cd apps\k7\backend-rs || exit /b 1
cargo clippy --locked --target-dir "%TARGET%" --workspace --all-targets -- -D warnings || exit /b 1
call :tests --workspace || exit /b 1
cd "%ROOT%"
:browserpart
echo [%AREAS%] | findstr /c:"browser" > nul || goto stats
rem The browser's crates, each its own Rust workspace (packages/song-schema/docs/wasmBoundary.md); rustfmt is not in every rustup profile.
rustup component add rustfmt > nul 2>&1
call :browser packages\song-schema\rust || exit /b 1
call :browser packages\k7-wasm\rust || exit /b 1
call :browser packages\song\rust || exit /b 1
rem song-edit has no ts feature: its tests (the winpath ones among them) run as they are.
pushd "%ROOT%\packages\song-edit\rust" || exit /b 1
call :tests || exit /b 1
popd
rem The song crate's browser build is not optional: the target is added here, not looked for.
rustup target add wasm32-unknown-unknown || exit /b 1
pushd "%ROOT%\packages\song\rust" || exit /b 1
cargo clippy --locked --target wasm32-unknown-unknown --no-default-features --target-dir "%TARGET%" -- -D warnings || exit /b 1
cargo build --locked --target wasm32-unknown-unknown --no-default-features --target-dir "%TARGET%" || exit /b 1
popd
rustup target list --installed 2> nul | findstr /c:"wasm32-unknown-unknown" > nul && (
  pushd "%ROOT%\packages\k7-wasm\rust" && cargo build --locked --release --target wasm32-unknown-unknown --target-dir "%TARGET%" && popd || exit /b 1
)
:stats
if defined RUSTC_WRAPPER "%RUSTC_WRAPPER%" --show-stats
exit /b 0

:browser
pushd "%ROOT%\%~1" || exit /b 1
cargo fmt --check || exit /b 1
cargo clippy --locked --target-dir "%TARGET%" --all-targets --features ts -- -D warnings || exit /b 1
call :tests || exit /b 1
popd
exit /b 0

rem The current folder's tests, every binary run to the end; nextest runs them all at once, and these crates hold no doctest it would miss.
:tests
cargo nextest --version > nul 2>&1 || goto plaintests
cargo nextest run --locked --target-dir "%TARGET%" --no-fail-fast %* || exit /b 1
exit /b 0
:plaintests
echo no cargo-nextest on this agent: cargo test instead
cargo test --locked --target-dir "%TARGET%" --no-fail-fast %* || exit /b 1
exit /b 0
