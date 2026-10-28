"""Check release AAB structure and ELF alignment before local upload-key signing."""
import argparse,struct,zipfile
from pathlib import Path

def verify(path):
 with zipfile.ZipFile(path) as archive:
  names=archive.namelist()
  assert 'BundleConfig.pb' in names and 'base/manifest/AndroidManifest.xml' in names,'Not an Android App Bundle'
  assert not any(n.upper().startswith('META-INF/') and n.upper().endswith(('.RSA','.DSA','.EC','.SF')) for n in names),'Expected an unsigned bundle before local signing'
  libraries=[n for n in names if n.startswith('base/lib/') and n.endswith('.so')]
  assert libraries and all(n.split('/')[2]=='arm64-v8a' for n in libraries),'Unexpected or missing native ABI'
  assert any('tv_manager' in n or 'app' in Path(n).name for n in libraries),'Missing application native library'
  for name in libraries:
   data=archive.read(name);assert data[:4]==b'\x7fELF',name
   endian='<' if data[5]==1 else '>';wide=data[4]==2
   offset=struct.unpack_from(endian+('Q' if wide else 'I'),data,32 if wide else 28)[0]
   size,count=struct.unpack_from(endian+'HH',data,54 if wide else 42)
   loads=0
   for index in range(count):
    header=offset+index*size
    if struct.unpack_from(endian+'I',data,header)[0]!=1:continue
    loads+=1
    alignment=struct.unpack_from(endian+('Q' if wide else 'I'),data,header+(48 if wide else 28))[0]
    assert alignment>=16384,(name,'Native library lacks 16 KB alignment',alignment)
   assert loads,name
  print(f'Unsigned AAB structure, arm64 ABI and 16 KB ELF alignment verified ({len(libraries)} libraries).')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('bundle',type=Path);verify(p.parse_args().bundle)
