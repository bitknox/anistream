# Diagnosis only (to be dropped): what this mpv reports while a start=180 load lands on a
# generated 200 s clip, and what keep-open an encode-mode child ends up with.
import json, os, socket, subprocess, tempfile, time

ver = subprocess.run(["mpv", "--version"], capture_output=True, text=True).stdout.split()[1].lstrip("v")
minor = int(ver.split(".")[1])
print("mpv", ver)


def player(args):
    d = tempfile.mkdtemp()
    sock = os.path.join(d, "s")
    proc = subprocess.Popen(["mpv", "--no-config", f"--input-ipc-server={sock}", "--msg-level=all=no"] + args)
    for _ in range(250):
        if os.path.exists(sock):
            break
        time.sleep(0.02)
    s = socket.socket(socket.AF_UNIX)
    s.connect(sock)
    return proc, s


def send(s, cmd, rid=None):
    msg = {"command": cmd}
    if rid is not None:
        msg["request_id"] = rid
    s.sendall((json.dumps(msg) + "\n").encode())


def lines(s, seconds):
    s.settimeout(0.05)
    buf, t0 = b"", time.monotonic()
    while time.monotonic() - t0 < seconds:
        try:
            chunk = s.recv(65536)
        except socket.timeout:
            continue
        if not chunk:
            return
        buf += chunk
        while b"\n" in buf:
            line, buf = buf.split(b"\n", 1)
            yield json.loads(line)


# 1. The binge's resume: idle window, loadfile with start=180, 200 s lavfi clip.
proc, s = player(["--idle=yes", "--vo=null", "--ao=null", "--force-window=no"])
for i, p in enumerate(["time-pos", "duration", "seeking"]):
    send(s, ["observe_property", i + 1, p])
url = "av://lavfi:testsrc=duration=200:size=320x240:rate=30"
t0 = time.monotonic()
if minor >= 38:
    send(s, ["loadfile", url, "replace", -1, {"start": "180"}])
else:
    send(s, ["loadfile", url, "replace", "start=%3%180"])
last = {}
for ev in lines(s, 2.5):
    t = time.monotonic() - t0
    if ev.get("event") == "property-change":
        name, data = ev["name"], ev.get("data")
        prev = last.get(name)
        last[name] = data
        # Every change of seeking, the first few of the rest, then one every ~0.1 s.
        if name == "seeking" or not isinstance(data, (int, float)) or prev is None \
                or int(t * 10) != int(last.get(name + "@", -1)):
            last[name + "@"] = t * 10
            print(f"  {t:6.3f}  {name} = {data}")
    elif ev.get("event"):
        print(f"  {t:6.3f}  event {ev['event']} {ev.get('reason', '')}")
send(s, ["quit"])
for ev in lines(s, 1.0):
    if ev.get("event"):
        print(f"  quit: {ev}")
proc.wait(5)

# 2. keep-open in encoding mode, set on the command line.
d = tempfile.mkdtemp()
proc, s = player(["--idle", "--pause", "--keep-open=always", "--ovc=rawvideo", "--of=image2",
                  "--ofopts=update=1", f"--o={d}/t.bgra", "--vf=format=bgra",
                  "av://lavfi:testsrc2=size=64x64:rate=10:duration=5"])
send(s, ["get_property", "keep-open"], 1)
for ev in lines(s, 2.0):
    if ev.get("request_id") == 1:
        print("encode-mode keep-open from the command line:", ev)
        break
send(s, ["quit"])
proc.wait(5)
