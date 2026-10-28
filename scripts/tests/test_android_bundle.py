import contextlib,io,pathlib,struct,tempfile,unittest,zipfile,sys
sys.path.insert(0,str(pathlib.Path(__file__).parents[1]))
from verify_android_bundle import verify

def elf(alignment):
 data=bytearray(120);data[:6]=b'\x7fELF\x02\x01'
 struct.pack_into('<Q',data,32,64);struct.pack_into('<HH',data,54,56,1)
 struct.pack_into('<I',data,64,1);struct.pack_into('<Q',data,112,alignment)
 return data
class AndroidBundleChecks(unittest.TestCase):
 def bundle(self,root,alignment=16384,abi='arm64-v8a',signed=False):
  path=pathlib.Path(root)/'test.aab'
  with zipfile.ZipFile(path,'w') as z:
   z.writestr('BundleConfig.pb',b'config');z.writestr('base/manifest/AndroidManifest.xml',b'manifest')
   z.writestr(f'base/lib/{abi}/libmain.so',elf(alignment))
   if signed:z.writestr('META-INF/UPLOAD.RSA',b'signature')
  return path
 def test_accepts_actual_dioxus_library_name_and_16k_alignment(self):
  with tempfile.TemporaryDirectory() as root,contextlib.redirect_stdout(io.StringIO()):verify(self.bundle(root))
 def test_rejects_four_kilobyte_native_load_segments(self):
  with tempfile.TemporaryDirectory() as root,self.assertRaises(AssertionError):verify(self.bundle(root,4096))
 def test_rejects_abis_without_the_application_payload(self):
  with tempfile.TemporaryDirectory() as root,self.assertRaises(AssertionError):verify(self.bundle(root,abi='x86_64'))
 def test_rejects_pre_signed_input_for_local_upload_signing(self):
  with tempfile.TemporaryDirectory() as root,self.assertRaises(AssertionError):verify(self.bundle(root,signed=True))
if __name__=='__main__':unittest.main()
