Name:           ward
Version:        0.1.0
Release:        1%{?dist}
Summary:        Native GTK firewall manager for UFW and firewalld
License:        GPL-3.0-or-later AND MIT AND Unicode-3.0 AND (Apache-2.0 WITH LLVM-exception)
Source0:        ward-%{version}.tar.gz
BuildRequires:  gcc
BuildRequires:  cargo >= 1.92
BuildRequires:  rust >= 1.92
BuildRequires:  pkgconfig(gtk4) >= 4.12
BuildRequires:  pkgconfig(libadwaita-1) >= 1.5
BuildRequires:  desktop-file-utils
BuildRequires:  add-determinism
Requires:       gtk4 >= 4.12
Requires:       libadwaita >= 1.5
Requires:       glib2
Requires:       /usr/bin/pkexec
Requires:       desktop-file-utils
Requires:       hicolor-icon-theme
Suggests:       firewalld

%description
Manage firewall status, rules, and port exceptions using a native
GTK4/libadwaita interface. UFW operations use a scoped PolicyKit helper;
firewalld operations use its system D-Bus API. Installing Ward does not
install, enable, disable, or switch firewall backends.

%prep
%autosetup -n ward-%{version}

%build
export CARGO_TARGET_DIR=%{_builddir}/ward-cargo
cargo build --release --locked --offline

%install
packaging/stage.sh %{_builddir}/ward-cargo/release %{buildroot}
desktop-file-validate %{buildroot}%{_datadir}/applications/io.github.ward.Ward.desktop
glib-compile-schemas --strict --dry-run %{buildroot}%{_datadir}/glib-2.0/schemas

# Fedora's glib2, desktop-file-utils and icon-cache file triggers update
# shared caches. No package script starts services or changes firewall state.
%files
%license /usr/share/licenses/ward/LICENSE
%doc /usr/share/doc/ward/README.md
%doc /usr/share/doc/ward/THIRD-PARTY-NOTICES
%{_bindir}/ward
/usr/libexec/ward-helper
%{_datadir}/applications/io.github.ward.Ward.desktop
%{_datadir}/icons/hicolor/scalable/apps/io.github.ward.Ward.svg
%{_datadir}/glib-2.0/schemas/io.github.ward.Ward.gschema.xml
%{_datadir}/polkit-1/actions/io.github.ward.Ward.policy

%changelog
* Thu Nov 26 2026 niruxxdaboi <perezkyle98@protonmail.com> - 0.1.0-1
- Initial native distribution packages.
