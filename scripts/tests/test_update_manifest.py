import base64, importlib.util, json, pathlib, tempfile, unittest
from unittest.mock import patch
ROOT=pathlib.Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('sign_updates',ROOT/'scripts/sign_update_manifest.py')
updates=importlib.util.module_from_spec(spec);spec.loader.exec_module(updates)
class UpdateManifestTests(unittest.TestCase):
    def release(self):
        version='1.0.0-beta.3';tag='v'+version
        suffixes=['windows-x86_64-setup.exe','macos-aarch64.pkg','macos-x86_64.pkg','linux-x86_64.deb','linux-x86_64.flatpak']
        return dict(tag_name=tag,draft=False,assets=[dict(name=f'Media-Manager-{version}-{suffix}',size=100,digest='sha256:'+'a'*64,browser_download_url=updates.ROOT+tag+f'/Media-Manager-{version}-{suffix}') for suffix in suffixes])
    def test_complete_release_binds_all_platform_hashes_and_urls(self):
        result=updates.manifest(self.release())
        self.assertEqual(result['version'],'1.0.0-beta.3');self.assertEqual(len(result['packages']),5)
        self.assertEqual(result['packages'][0]['sha256'],'a'*64)
    def test_incomplete_release_and_missing_digest_fail_closed(self):
        r=self.release();r['assets'].pop()
        with self.assertRaisesRegex(ValueError,'incomplete'):updates.manifest(r)
        r=self.release();r['assets'][0]['digest']=''
        with self.assertRaisesRegex(ValueError,'digest'):updates.manifest(r)
    def test_untrusted_urls_drafts_and_invalid_tags_are_rejected(self):
        for field,value in [('tag_name','../../evil'),('draft',True)]:
            r=self.release();r[field]=value
            with self.assertRaises(ValueError):updates.manifest(r)
        r=self.release();r['assets'][0]['browser_download_url']='https://example.com/update.exe'
        with self.assertRaises(ValueError):updates.manifest(r)
    def test_signing_refuses_a_key_that_is_not_pinned(self):
        with patch.object(updates.subprocess,'check_output',return_value=b'wrong public key'):
            with self.assertRaisesRegex(ValueError,'does not match'):updates.sign(b'{}','fixture-not-a-key',b'0'*32)
if __name__=='__main__':unittest.main()
