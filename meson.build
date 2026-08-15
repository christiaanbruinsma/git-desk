project(
  'git-desk',
  version: '0.9.0',
  meson_version: '>= 1.3.0',
)

prefix = get_option('prefix')
bindir = prefix / get_option('bindir')
datadir = prefix / get_option('datadir')
cargo = find_program('cargo')
gnome = import('gnome')
i18n = import('i18n')
localedir = datadir / 'locale'

base_app_id = 'io.github.christiaanbruinsma.GitDesk'
profile = get_option('profile')
if profile == 'development'
  app_id = base_app_id + '.Devel'
else
  app_id = base_app_id
endif

profile_conf = configuration_data()
profile_conf.set('APP_ID', app_id)

desktop_file = configure_file(
  input: 'data/io.github.christiaanbruinsma.GitDesk.desktop.in',
  output: app_id + '.desktop',
  configuration: profile_conf,
)

metainfo_file = configure_file(
  input: 'data/io.github.christiaanbruinsma.GitDesk.metainfo.xml.in',
  output: app_id + '.metainfo.xml',
  configuration: profile_conf,
)

rust_sources = files(
  'src/main.rs',
  'src/app.rs',
  'src/git/backend.rs',
  'src/i18n.rs',
  'src/git/graph.rs',
  'src/git/mod.rs',
  'src/git/models.rs',
  'src/git/parser.rs',
  'src/providers/mod.rs',
  'src/services/mod.rs',
  'src/services/portal.rs',
  'src/services/recent.rs',
  'src/services/watcher.rs',
  'src/ui/diff_view.rs',
  'src/ui/mod.rs',
  'src/ui/repository.rs',
  'src/ui/welcome.rs',
  'src/ui/window.rs',
)

cargo_inputs = rust_sources + files('Cargo.toml', 'Cargo.lock')

git_desk = custom_target(
  'git-desk',
  input: cargo_inputs,
  output: 'git-desk',
  command: [
    'sh', '-c',
    'LOCALEDIR="$4" GIT_DESK_APP_ID="$5" cargo build --release --manifest-path "$1" --target-dir "$2" && cp "$2/release/git-desk" "$3"',
    'sh',
    meson.project_source_root() / 'Cargo.toml',
    meson.project_build_root() / 'cargo-target',
    '@OUTPUT@',
    localedir,
    app_id,
  ],
  build_by_default: true,
  install: true,
  install_dir: bindir,
)

subdir('po')

install_data(
  desktop_file,
  install_dir: datadir / 'applications',
)

install_data(
  metainfo_file,
  install_dir: datadir / 'metainfo',
)

install_data(
  'data/icons/hicolor/scalable/actions/panel-right-symbolic.svg',
  'data/icons/hicolor/scalable/actions/document-edit-symbolic.svg',
  install_dir: datadir / 'icons/hicolor/scalable/actions',
)

install_data(
  'data/icons/hicolor/scalable/status/starred-symbolic.svg',
  'data/icons/hicolor/scalable/status/non-starred-symbolic.svg',
  install_dir: datadir / 'icons/hicolor/scalable/status',
)

install_data(
  'data/icons/hicolor/scalable/apps/io.github.christiaanbruinsma.GitDesk.svg',
  install_dir: datadir / 'icons/hicolor/scalable/apps',
  rename: app_id + '.svg',
)

install_data('README.md', install_dir: datadir / 'doc' / meson.project_name())
install_data('LICENSE', install_dir: datadir / 'licenses' / app_id)

gnome.post_install(
  gtk_update_icon_cache: true,
  update_desktop_database: true,
)
