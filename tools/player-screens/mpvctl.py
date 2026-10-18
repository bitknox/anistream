"""Send commands to mpv's JSON IPC socket: mpvctl.py SOCKET 'json-array' ['json-array' ...]"""
import json, socket, sys

def call(sock_path, *commands):
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.connect(sock_path)
    f = s.makefile("rwb")
    out = []
    for i, command in enumerate(commands, 1):
        f.write((json.dumps({"command": command, "request_id": i}) + "\n").encode())
        f.flush()
        while True:
            reply = json.loads(f.readline())
            if reply.get("request_id") == i:
                out.append(reply)
                break
    s.close()
    return out

if __name__ == "__main__":
    for reply in call(sys.argv[1], *(json.loads(a) for a in sys.argv[2:])):
        print(json.dumps(reply))
