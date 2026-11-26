project('ward', version: '0.1.0', meson_version: '>= 1.0.0', license: 'GPL-3.0-or-later', default_options: ['prefix=/usr'])
if get_option('prefix') != '/usr'
  error('The PolicyKit helper uses /usr/libexec/ward-helper; configure with --prefix=/usr')
endif
dependency('gtk4', version: '>= 4.12')
dependency('libadwaita-1', version: '>= 1.5')
find_program('cargo')
custom_target('ward-binaries',
  output: ['ward', 'ward-helper'],
  command: [find_program('build-aux/cargo-build.sh'), meson.project_source_root(), meson.current_build_dir()],
  build_by_default: true,
  build_always_stale: true,
  install: true,
  install_dir: [get_option('bindir'), 'libexec'],
)
install_data('data/io.github.ward.Ward.desktop', install_dir: get_option('datadir') / 'applications')
install_data('data/io.github.ward.Ward.svg', install_dir: get_option('datadir') / 'icons/hicolor/scalable/apps')
install_data('data/io.github.ward.Ward.gschema.xml', install_dir: get_option('datadir') / 'glib-2.0/schemas')
install_data('data/io.github.ward.Ward.policy', install_dir: get_option('datadir') / 'polkit-1/actions')
gnome = import('gnome')
gnome.post_install(glib_compile_schemas: true, gtk_update_icon_cache: true, update_desktop_database: true)
