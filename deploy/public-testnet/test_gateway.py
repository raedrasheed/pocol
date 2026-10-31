#!/usr/bin/env python3
"""Real local Nginx TLS gateway: trusted TLS, method/path/body/host and rate controls.

Requires nginx and openssl. Uses ephemeral loopback ports and a temporary test CA.
"""
import http.client
import http.server
import json
from pathlib import Path
import shutil
import socket
import ssl
import subprocess
import tempfile
import threading
import time
from prepare import nginx


def main():
    executable = shutil.which('nginx')
    if not executable: raise RuntimeError('nginx is required; gateway test is not skipped')
    received = []
    class Backend(http.server.BaseHTTPRequestHandler):
        def do_POST(self):
            body = self.rfile.read(int(self.headers['Content-Length']))
            received.append((self.headers.get('Origin'), self.headers.get('X-Forwarded-For')))
            answer = json.dumps({'jsonrpc':'2.0', 'id':json.loads(body)['id'], 'result':'ok'}).encode()
            self.send_response(200); self.send_header('Content-Length', str(len(answer)))
            self.end_headers(); self.wfile.write(answer)
        def log_message(self, *args): pass
    backend = http.server.HTTPServer(('127.0.0.1', 0), Backend)
    thread = threading.Thread(target=backend.serve_forever, daemon=True); thread.start()
    with tempfile.TemporaryDirectory(prefix='pocol-gateway-') as temp:
        root = Path(temp); (root / 'logs').mkdir(); cert = root / 'cert.pem'; key = root / 'key.pem'
        subprocess.run(['openssl','req','-x509','-newkey','rsa:2048','-nodes','-days','1','-subj','/CN=127.0.0.1',
                        '-addext','subjectAltName=IP:127.0.0.1','-keyout',str(key),'-out',str(cert)],
                       check=True, capture_output=True)
        with socket.socket() as held: held.bind(('127.0.0.1',0)); port = held.getsockname()[1]
        conf = nginx({'rpcHost':'rpc.example.test', 'rpcPort':backend.server_port})
        conf = conf.replace('listen 443 ssl default_server;', f'listen 127.0.0.1:{port} ssl default_server;')
        conf = conf.replace('/run/nginx-pocol.pid', str(root / 'nginx.pid')).replace('/var/log/nginx/pocol-error.log', str(root / 'error.log'))
        conf = conf.replace('/etc/pocol-testnet/tls/fullchain.pem', str(cert)).replace('/etc/pocol-testnet/tls/privkey.pem', str(key))
        conf = conf.replace('http {', f'http {{\n client_body_temp_path {root}/body;\n proxy_temp_path {root}/proxy;')
        path = root / 'nginx.conf'; path.write_text(conf)
        subprocess.run([executable,'-t','-p',temp,'-c',str(path)], check=True, capture_output=True)
        proc = subprocess.Popen([executable,'-p',temp,'-c',str(path),'-g','daemon off;'], stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
        context = ssl.create_default_context(cafile=str(cert))
        def request(method='POST', path='/', host='rpc.example.test', body=None, headers=None):
            conn = http.client.HTTPSConnection('127.0.0.1', port, context=context, timeout=5)
            try:
                conn.request(method, path, body if body is not None else '{"jsonrpc":"2.0","id":1,"method":"eth_chainId","params":[]}',
                             {'Host':host, 'Content-Type':'application/json', **(headers or {})})
                response = conn.getresponse(); response.read(); return response.status
            finally: conn.close()
        try:
            until = time.monotonic()+10
            while True:
                if proc.poll() is not None: raise RuntimeError('Nginx exited: ' + proc.stderr.read().decode())
                try:
                    if request() == 200: break
                except (OSError, http.client.HTTPException): pass
                if time.monotonic() >= until: raise RuntimeError('Gateway did not start')
                time.sleep(.1)
            assert request(headers={'Origin':'https://untrusted.example','X-Forwarded-For':'1.2.3.4'}) == 200
            assert received[-1] == ('https://untrusted.example', None), received[-1]
            assert request(method='GET') == 405
            assert request(path='/not-rpc') == 404
            assert request(body='x' * (256*1024+1)) == 413
            try: request(host='wrong.example')
            except http.client.RemoteDisconnected: pass
            else: raise AssertionError('Wrong host was not closed')
            answers = [request(headers={'X-Forwarded-For':f'198.51.100.{i}'}) for i in range(12)]
            assert 429 in answers, answers
            print(json.dumps({'result':'PASS','scope':'local TLS gateway with mock RPC backend',
                'checks':['trusted TLS','origin retained','forwarded IP stripped','method/path/body/host refusal','per-IP rate limit across new connections'],
                'statuses':answers,'publicLaunchReady':False}))
        finally:
            proc.terminate()
            try: proc.wait(timeout=10)
            except subprocess.TimeoutExpired: proc.kill(); proc.wait(timeout=5)
            backend.shutdown(); backend.server_close(); thread.join(timeout=5)


if __name__ == '__main__': main()
