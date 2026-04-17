name := 'cosmic-maps'
export APPID := 'com.system76.CosmicMaps'

rootdir := ''
prefix := '/usr'

base-dir := absolute_path(clean(rootdir + prefix))

bin-dir := base-dir / 'bin'
share-dir := base-dir / 'share'
applications-dir := share-dir / 'applications'
icons-dir := share-dir / 'icons' / 'hicolor' / 'scalable' / 'apps'

export CARGO_TARGET_DIR := env('CARGO_TARGET_DIR', 'target')
bin-path := CARGO_TARGET_DIR / 'release' / name

build:
    cargo build --release

build-debug:
    cargo build

install:
    install -Dm0755 {{bin-path}} {{bin-dir}}/{{name}}
    install -Dm0644 resources/{{APPID}}.desktop {{applications-dir}}/{{APPID}}.desktop
    install -Dm0644 resources/icons/hicolor/scalable/apps/{{APPID}}.svg {{icons-dir}}/{{APPID}}.svg

uninstall:
    rm -f {{bin-dir}}/{{name}}
    rm -f {{applications-dir}}/{{APPID}}.desktop
    rm -f {{icons-dir}}/{{APPID}}.svg

clean:
    cargo clean

run:
    cargo run
