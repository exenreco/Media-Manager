"""Check the merged APK manifest rather than only the source manifest."""
import sys,xml.etree.ElementTree as ET
from pathlib import Path
android='{http://schemas.android.com/apk/res/android}'
root=ET.parse(Path(sys.argv[1])).getroot()
activities={a.get(android+'name'):a for a in root.findall('./application/activity')}
for name in ['dev.dioxus.main.MainActivity','dev.dioxus.main.PairingCaptureActivity']:
 a=activities[name]
 assert a.get(android+'resizeableActivity')=='true',(name,'must be resizable')
 assert a.get(android+'screenOrientation') in [None,'unspecified','fullUser','fullSensor'],(name,'orientation restriction')
 assert a.get(android+'maxAspectRatio') is None,(name,'aspect ratio restriction')
assert 'com.journeyapps.barcodescanner.CaptureActivity' not in activities,'Unused portrait-only scanner must be removed'
print('Merged app and scanner activities are resizable and have no orientation/aspect restrictions.')
