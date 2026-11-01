import json, os, subprocess, time, urllib.request
WD="http://127.0.0.1:4444"
def call(m,p,b=None):
    r=urllib.request.Request(WD+p,data=json.dumps(b).encode() if b is not None else None,method=m,headers={"Content-Type":"application/json"})
    with urllib.request.urlopen(r,timeout=120) as x: return json.load(x)["value"]
sid=call("POST","/session",{"capabilities":{"alwaysMatch":{"webkitgtk:browserOptions":{"binary":"/usr/bin/archdroid","args":[]}}}})["sessionId"]
js=lambda s: call("POST",f"/session/{sid}/execute/sync",{"script":s,"args":[]})
sub=lambda: js("return document.body.innerText.match(/\\d+ applications? ready to launch/)?.[0]")
log={}
t0=time.time()
while time.time()-t0<300:
    s=sub()
    if s and not s.startswith("0 "): break
    time.sleep(2)
log["apps_appeared"]=(round(time.time()-t0), sub(), len(os.listdir(os.path.expanduser("~/.local/share/applications"))))
subprocess.run([os.path.expanduser("~/dnd.sh"), os.path.expanduser("~/test-data/F-Droid.apk"), "80-fedora-drop"], capture_output=True)
end=time.time()+120
while time.time()<end and not js("return /APK Installed|Install Failed|Install Error/.test(document.body.innerText)"): time.sleep(1)
log["drop"]=js("return document.body.innerText.match(/APK Installed|Install Failed[^\\n]*|Install Error[^\\n]*/)?.[0]")
time.sleep(3); log["after_drop"]=(sub(), js("return document.body.innerText.match(/User Apps & Games\\s*\\d+/)?.[0]"))
js("[...document.querySelectorAll('button')].find(b => b.textContent.includes('Stop Engine')).click()")
end=time.time()+90
while time.time()<end and not js("return /Engine Stopped/.test(document.body.innerText)"): time.sleep(1)
log["stopped"]=subprocess.run(["waydroid","status"],capture_output=True,text=True).stdout.splitlines()[0]
print(json.dumps(log)); call("DELETE",f"/session/{sid}")
