#!/usr/bin/env python3
"""Create a checksummed, vendored source archive and render the Arch recipe."""
from pathlib import Path
import gzip
import hashlib
import os
import shutil
import subprocess
import tarfile
import tempfile
import time
import tomllib

repo = Path(__file__).resolve().parent.parent
version = tomllib.loads((repo/'Cargo.toml').read_text())['package']['version']
output = Path(os.environ.get('WARD_OUTPUT_DIR', repo/'dist')).resolve()
output.mkdir(parents=True, exist_ok=True)
work = repo/'target/package-build'
work.mkdir(parents=True, exist_ok=True)
epoch = int(os.environ.get('SOURCE_DATE_EPOCH', time.time()))
with tempfile.TemporaryDirectory(prefix='source-', dir=work) as temporary:
    source = Path(temporary)/f'ward-{version}'
    source.mkdir()
    for name in ['src','data','tests','packaging','build-aux','scripts','docs']:
        shutil.copytree(repo/name, source/name, ignore=shutil.ignore_patterns('gschemas.compiled','__pycache__','*.pyc'))
    for name in ['Cargo.toml','Cargo.lock','build.rs','meson.build','README.md','LICENSE']:
        shutil.copyfile(repo/name, source/name)
    (source/'.cargo').mkdir()
    with (source/'.cargo/config.toml').open('w') as config:
        subprocess.run(['cargo','vendor','--locked','--versioned-dirs','vendor'], cwd=source, stdout=config, check=True)
    notices=['Ward third-party Rust dependency notices\n', 'The following upstream notices are retained from the vendored source.\n']
    for crate in sorted((source/'vendor').iterdir()):
        manifest=tomllib.loads((crate/'Cargo.toml').read_text())
        package=manifest['package']
        notices.append(f"\n{'='*72}\n{package['name']} {package['version']}\nLicense: {package.get('license', 'See upstream license file')}\n")
        for file in sorted(crate.iterdir()):
            if file.is_file() and file.name.upper().startswith(('LICENSE','COPYING','COPYRIGHT','NOTICE')):
                notices.append(f"\n--- {file.name} ---\n{file.read_text(errors='replace')}\n")
    (source/'THIRD-PARTY-NOTICES').write_text(''.join(notices))
    def normalize(info):
        info.uid=info.gid=0
        info.uname=info.gname='root'
        info.mtime=epoch
        return info
    archive=output/f'ward-{version}.tar.gz'
    with archive.open('wb') as raw, gzip.GzipFile(filename='',fileobj=raw,mode='wb',mtime=epoch) as zipped:
        with tarfile.open(fileobj=zipped,mode='w') as tar:
            tar.add(source,arcname=source.name,filter=normalize)
    checksum=hashlib.file_digest(archive.open('rb'),'sha256').hexdigest()
    recipe=(repo/'packaging/arch/PKGBUILD.in').read_text().replace('@SOURCE_SHA256@',checksum)
    (output/'PKGBUILD').write_text(recipe)
    (output/'SOURCE_DATE_EPOCH').write_text(str(epoch)+'\n')
    (output/(archive.name+'.sha256')).write_text(f'{checksum}  {archive.name}\n')
    print(f'Created {archive.name} with vendored dependencies and SHA-256 {checksum}')
