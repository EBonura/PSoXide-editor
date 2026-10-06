"""Minimal stdio MCP client for psxed-mcp: run a batch of tool calls in ONE server process
(one process = one staged edit batch), print text results, save image results.

  python3 mcpc.py --project DIR --tools                 # list tools + input schemas
  python3 mcpc.py --project DIR calls.json [--out DIR]  # calls.json: [{"tool": name, "args": {...}}, ...]
"""
import json, subprocess, sys, base64, pathlib, argparse

REPO = pathlib.Path(__file__).resolve().parents[2]
BIN = REPO / 'target/release/psxed-mcp' if (REPO / 'target/release/psxed-mcp').exists() else REPO / 'target/debug/psxed-mcp'
FRONTEND = REPO / 'target/release/frontend' if (REPO / 'target/release/frontend').exists() else REPO / 'target/run-fast/frontend'


class Client:
    def __init__(self, project):
        self.p = subprocess.Popen([str(BIN), '--project', str(project), '--frontend', str(FRONTEND)],
                                  cwd=REPO, stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, bufsize=1)
        self.n = 0
        self.req('initialize', {'protocolVersion': '2025-06-18', 'capabilities': {},
                                'clientInfo': {'name': 'mcpc', 'version': '0'}})
        self.send({'jsonrpc': '2.0', 'method': 'notifications/initialized'})

    def send(self, msg):
        self.p.stdin.write(json.dumps(msg) + '\n'); self.p.stdin.flush()

    def req(self, method, params):
        self.n += 1
        self.send({'jsonrpc': '2.0', 'id': self.n, 'method': method, 'params': params})
        while True:
            line = self.p.stdout.readline()
            if not line:
                raise RuntimeError('server closed the pipe')
            msg = json.loads(line)
            if msg.get('id') == self.n:
                if 'error' in msg:
                    raise RuntimeError(json.dumps(msg['error']))
                return msg['result']

    def call(self, tool, args):
        return self.req('tools/call', {'name': tool, 'arguments': args})

    def close(self):
        self.p.stdin.close(); self.p.wait(timeout=30)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--project', required=True)
    ap.add_argument('--tools', action='store_true')
    ap.add_argument('--out', default='.')
    ap.add_argument('calls', nargs='?')
    a = ap.parse_args()
    c = Client(a.project)
    try:
        if a.tools:
            for t in c.req('tools/list', {})['tools']:
                props = t.get('inputSchema', {}).get('properties', {})
                print(f"## {t['name']}: {t.get('description', '').strip()}")
                for k, v in props.items():
                    print(f"   - {k}: {v.get('type', '')} {v.get('description', '')}".rstrip())
            return
        calls = json.load(open(a.calls)) if a.calls != '-' else json.load(sys.stdin)
        out = pathlib.Path(a.out); out.mkdir(parents=True, exist_ok=True)
        for i, c_ in enumerate(calls):
            if c_['tool'] == '_clear_lights':
                import re
                lt = c.call('lights', {})['content'][0]['text']
                nl = len(re.findall(r'^- "', lt, re.M))
                for _ in range(nl):
                    c.call('delete_node', {'node': 'Point Light'})
                names = re.findall(r'^- "([^"]+)"', lt, re.M)
                for nm in names:
                    if nm != 'Point Light':
                        c.call('delete_node', {'node': nm})
                print(f'=== [{i}] _clear_lights: {nl} lights'); continue
            if c_['tool'] == '_clear':          # delete every brush: each round rebuilds from zero
                import re
                txt = c.call('status', {})['content'][0]['text']
                n = int(re.search(r'(\d+) brushes', txt).group(1))
                if n:
                    c.call('delete', {'first': 0, 'count': n})
                # and every point light: the loop owns all lighting in its test project
                lt = c.call('lights', {})['content'][0]['text']
                nl = len(re.findall(r'^- "', lt, re.M))
                for _ in range(nl):
                    c.call('delete_node', {'node': 'Point Light'})
                print(f'=== [{i}] _clear: deleted {n} brushes, {nl} lights')
                continue
            try:
                r = c.call(c_['tool'], c_.get('args', {}))
            except RuntimeError as e:
                if c_.get('stop_on_error', True):
                    raise
                print(f"=== [{i}] {c_['tool']} SKIPPED: {e}"); continue
            flag = ' ERROR' if r.get('isError') else ''
            print(f"=== [{i}] {c_['tool']}{flag}")
            for k, blk in enumerate(r.get('content', [])):
                if blk['type'] == 'text':
                    print(blk['text'])
                elif blk['type'] == 'image':
                    name = c_.get('save', f"{i:02d}_{c_['tool']}_{k}") + '.png'
                    (out / name).write_bytes(base64.b64decode(blk['data']))
                    print(f'[image saved {out / name}]')
            if r.get('isError') and c_.get('stop_on_error', True):
                sys.exit(1)
    finally:
        c.close()


if __name__ == '__main__':
    main()
