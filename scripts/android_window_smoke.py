"""Check real WebView safe-area geometry through rotations on a disposable emulator."""
import argparse,json,pathlib,subprocess,time
p=argparse.ArgumentParser(description=__doc__)
p.add_argument('--adb',required=True);p.add_argument('--device',default='emulator-5554')
p.add_argument('--evidence',type=pathlib.Path,required=True);a=p.parse_args();a.evidence.mkdir(parents=True,exist_ok=True)
base=[a.adb,'-s',a.device]
def run(*args):return subprocess.check_output(base+list(args),text=True).strip()
package='com.tvshowmanager.app'
run('shell','am','force-stop',package)
run('shell','am','start','-W','-n',package+'/dev.dioxus.main.MainActivity','--ez','media_manager_window_fixture','true')
natural_landscape=None
try:
 for rotation in [0,1,3,0]:
  run('shell','cmd','window','user-rotation','lock',str(rotation))
  time.sleep(2)
  for _ in range(20):
   raw=run('shell','run-as',package,'cat','files/window-insets-fixture.json')
   snapshot=json.loads(raw)
   if snapshot.get('dock') and snapshot.get('toolbar') and snapshot['width']>0:
    if natural_landscape is None or (snapshot['width']>snapshot['height']) == (natural_landscape != (rotation in [1,3])):break
   time.sleep(.5)
  (a.evidence/f'rotation-{rotation}.json').write_text(json.dumps(snapshot,indent=2))
  if natural_landscape is None:natural_landscape=snapshot['width']>snapshot['height']
  assert (snapshot['width']>snapshot['height']) == (natural_landscape != (rotation in [1,3])),snapshot
  safe=snapshot['safe']
  for key in ['toolbar','dock']:
   r=snapshot[key];assert r and r['right']>r['left'] and r['bottom']>r['top'],snapshot
   assert r['left']>=safe['left']-2 and r['right']<=snapshot['width']-safe['right']+2,snapshot
   assert r['top']>=safe['top']-2 and r['bottom']<=snapshot['height']-safe['bottom']+2,snapshot
  assert run('shell','pidof',package),'App exited during rotation'
 print('Toolbar and footer remain inside native safe edges in portrait and both landscape rotations.')
finally:
 run('shell','cmd','window','user-rotation','free')
 run('shell','am','force-stop',package)
