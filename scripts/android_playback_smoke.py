"""Exercise bundled Android decoding using private synthetic MP4 fixtures."""
import argparse,json,pathlib,subprocess,time
p=argparse.ArgumentParser();p.add_argument('--inline',action='store_true');p.add_argument('--codec',choices=['mpeg4','h264','h264-10bit'],default='mpeg4');p.add_argument('--adb',required=True);p.add_argument('--device',default='emulator-5554');p.add_argument('--evidence',type=pathlib.Path,required=True);a=p.parse_args();a.evidence.mkdir(parents=True,exist_ok=True)
fixture=a.evidence/'compatibility-fixture.mp4'
video=['-c:v','mpeg4','-q:v','4'] if a.codec=='mpeg4' else ['-c:v','libx264','-preset','ultrafast','-pix_fmt','yuv420p10le' if a.codec=='h264-10bit' else 'yuv420p']
subprocess.run(['ffmpeg','-nostdin','-v','error','-f','lavfi','-i','testsrc2=size=320x180:rate=24','-f','lavfi','-i','sine=frequency=440:sample_rate=48000','-t','12',*video,'-c:a','aac','-b:a','96k','-y',str(fixture)],check=True)
base=[a.adb,'-s',a.device]
def run(*args):return subprocess.check_output(base+list(args),text=True).strip()
run('shell','am','force-stop','com.tvshowmanager.app')
run('push',str(fixture),'/data/local/tmp/media-manager-codec-fixture.mp4')
run('shell','run-as','com.tvshowmanager.app','cp','/data/local/tmp/media-manager-codec-fixture.mp4','files/compatibility-fixture.mp4')
run('shell','run-as','com.tvshowmanager.app','rm','-f','files/compatibility-fixture-events.txt')
print(run('shell','am','start','-W','-n','com.tvshowmanager.app/dev.dioxus.main.MainActivity','--ez','media_manager_preview_fixture','true','--ez','media_manager_inline_fixture',str(a.inline).lower()),flush=True)
events=''
for _ in range(30):
 time.sleep(2)
 result=subprocess.run(base+['shell','run-as','com.tvshowmanager.app','cat','files/compatibility-fixture-events.txt'],capture_output=True,text=True)
 events=result.stdout
 if 'ended' in events or 'error' in events:break
(a.evidence/'events.txt').write_text(events)
pid=run('shell','pidof','com.tvshowmanager.app')
assert pid,'App exited during codec test'
log=run('logcat','-d','--pid',pid.split()[0],'-t','1500');(a.evidence/'app-log.txt').write_text(log,encoding='utf-8')
assert 'error' not in events.splitlines(),events
assert 'video-output' in events.splitlines(),events
assert 'playing' in events.splitlines(),events
assert 'ended' in events.splitlines(),'Playback did not advance to completion: '+events
assert ('mode:inline' if a.inline else 'mode:fullscreen') in events.splitlines(),events
assert not any(s in log for s in ['FATAL EXCEPTION','Fatal signal','ANR in com.tvshowmanager.app'])
print(f'{a.codec}/AAC MP4 at 24 fps produced native video output ({"inline" if a.inline else "fullscreen"}); app remained running.')
print(events)
run('shell','am','force-stop','com.tvshowmanager.app')
run('shell','am','start','-W','-n','com.tvshowmanager.app/dev.dioxus.main.MainActivity','--ez','media_manager_preview_fixture','false')
