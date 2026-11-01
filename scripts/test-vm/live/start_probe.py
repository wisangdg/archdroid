import json, time, urllib.request, sys
WD="http://127.0.0.1:4444"
def call(m,p,b=None):
    r=urllib.request.Request(WD+p,data=json.dumps(b).encode() if b is not None else None,method=m,headers={"Content-Type":"application/json"})
    with urllib.request.urlopen(r,timeout=120) as x: return json.load(x)["value"]
sid=call("POST","/session",{"capabilities":{"alwaysMatch":{"webkitgtk:browserOptions":{"binary":sys.argv[1],"args":[]}}}})["sessionId"]
js=lambda s: call("POST",f"/session/{sid}/execute/sync",{"script":s,"args":[]})
time.sleep(7)
js("[...document.querySelectorAll('button')].find(b => b.textContent.includes('Start Engine')).click()")
t0=time.time(); out=None
while time.time()-t0 < 60:
    t=js("return [...document.querySelectorAll('.toast-item')].map(e=>e.innerText.replace(/\\s+/g,' ')).join(' | ')")
    if "Start Failed" in t or "Slow to Start" in t: out=("toast", t); break
    if js("return /Engine Running/.test(document.body.innerText)"): out=("running", None); break
    time.sleep(0.5)
print(json.dumps({"seconds": round(time.time()-t0,1), "result": out}))
call("DELETE",f"/session/{sid}")
