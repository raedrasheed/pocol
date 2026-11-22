"""The explorer gateway's request budget (EXP-02): a bucket per client inside the node's bucket, and a share of the
node's bucket that public /rpc reads cannot take, kept for the explorer site's /api reads and pocol.net's summary.
Clients are told apart by the last X-Forwarded-For hop of a loopback (Caddy) peer. Loopback only."""
import json
import sys
import threading
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import gateway  # noqa: E402
import explorer_fakes as F  # noqa: E402


class Headers:
    def __init__(self, *values):
        self.values = list(values)

    def get_all(self, name, default=None):
        return self.values if name == 'X-Forwarded-For' and self.values else default


class ClientKeyTests(unittest.TestCase):
    def test_the_last_forwarded_hop_of_a_loopback_peer(self):
        self.assertEqual(gateway.client_key('127.0.0.1', Headers('198.51.100.7')), '198.51.100.7')
        self.assertEqual(gateway.client_key('127.0.0.1', Headers('203.0.113.1, 198.51.100.7')), '198.51.100.7')
        self.assertEqual(gateway.client_key('127.0.0.1', Headers('203.0.113.1', '198.51.100.8')), '198.51.100.8')
        self.assertEqual(gateway.client_key('::1', Headers('198.51.100.7')), '198.51.100.7')
        self.assertEqual(gateway.client_key('127.0.0.1', Headers()), '127.0.0.1')
        self.assertEqual(gateway.client_key('127.0.0.1', Headers('not-an-ip')), 'unknown')

    def test_a_remote_peer_is_itself_and_ipv6_counts_by_its_64(self):
        self.assertEqual(gateway.client_key('198.51.100.9', Headers('203.0.113.1')), '198.51.100.9', 'only a loopback peer is trusted')
        self.assertEqual(gateway.client_key('127.0.0.1', Headers('2001:db8:1:2:aaaa::1')), '2001:db8:1:2::/64')
        self.assertEqual(gateway.client_key('127.0.0.1', Headers('2001:db8:1:2:bbbb::9')), '2001:db8:1:2::/64')


class BudgetTests(unittest.TestCase):
    def setUp(self):
        self.node = F.FakeNode()
        self.gw = gateway.Gateway(self.node.server_address[1], None, None, network=F.NET)
        self.server = gateway.Server(('127.0.0.1', 0), self.gw, 'http://127.0.0.1:8661')
        self.port = self.server.server_address[1]
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        self.clock = patch.object(gateway.time, 'monotonic', return_value=1000.0)  # no refill during a test
        self.clock.start()
        with self.gw.lock:
            self.gw.tokens, self.gw.token_time = gateway.GLOBAL_BURST, 1000.0

    def tearDown(self):
        self.clock.stop()
        self.server.shutdown(); self.server.server_close(); self.thread.join()
        self.node.stop()

    def rpc(self, client, path='/rpc'):
        value = {'jsonrpc': '2.0', 'id': 1, 'method': 'eth_blockNumber', 'params': []}
        return F.post_json(self.port, path, value, extra=[('X-Forwarded-For', client)])[0]

    def flood(self, client, path='/rpc', limit=200):
        """Requests of one token until the first 429: the number answered."""
        for n in range(limit):
            if self.rpc(client, path) == 429:
                return n
        return limit

    def test_one_flooding_client_does_not_starve_the_others(self):
        self.assertEqual(self.flood('198.51.100.1'), gateway.CLIENT_BURST)
        self.assertEqual(self.rpc('198.51.100.2'), 200, 'another client is still answered on /rpc')
        self.assertEqual(self.rpc('198.51.100.2', '/api/rpc.php'), 200, 'and on the explorer reads')
        self.assertEqual(self.rpc('198.51.100.1', '/api/rpc.php'), 429, 'the flooding client is limited everywhere')

    def test_public_rpc_cannot_take_the_explorer_share(self):
        answered = sum(self.flood(f'198.51.100.{i}') for i in range(10, 14))
        self.assertEqual(answered, gateway.GLOBAL_BURST - gateway.API_RESERVE)
        self.assertEqual(self.rpc('198.51.100.50'), 429, 'a new /rpc client gets no more than the public share')
        self.assertEqual(self.rpc('198.51.100.50', '/api/rpc.php'), 200, 'the explorer reads keep their share')
        status, _, raw = F.send(self.port, 'GET', '/api/health.php', [('X-Forwarded-For', '198.51.100.51')])
        self.assertEqual((status, json.loads(raw)['result']), (200, 'PASS'))
        # The /api reads may use the reserve down to zero.
        self.assertEqual(self.flood('198.51.100.60', '/api/rpc.php') + self.flood('198.51.100.61', '/api/rpc.php'),
                         gateway.API_RESERVE - 2)

    def test_a_refused_request_is_not_charged_and_buckets_refill(self):
        self.assertEqual(self.flood('198.51.100.1'), gateway.CLIENT_BURST)
        with self.gw.lock:
            left = self.gw.tokens
        self.assertEqual(self.rpc('198.51.100.1'), 429)
        with self.gw.lock:
            self.assertEqual(self.gw.tokens, left, 'a refused request takes nothing from the node bucket')
        self.clock.stop()
        self.clock = patch.object(gateway.time, 'monotonic', return_value=1000.0 + 1.0)
        self.clock.start()
        self.assertEqual(self.flood('198.51.100.1'), gateway.CLIENT_RATE, 'one second refills CLIENT_RATE tokens')

    def test_the_client_table_is_bounded(self):
        with patch.object(gateway, 'CLIENTS_MAX', 8):
            for i in range(20):
                self.assertTrue(self.gw.charge(1, f'client-{i}'))
            self.assertEqual(len(self.gw.clients), 8)
            self.assertEqual(list(self.gw.clients)[0], 'client-12', 'the least recently seen clients go first')


if __name__ == '__main__':
    unittest.main()
