// A stand-in for the wallet's Rust side, to look at the window in a plain
// browser: tools/ui-preview/serve.py injects it before app.js. Nothing here
// is real: no keys, no network. Pick what to show in the address bar:
//
//   ?state=wallet         a wallet in use, with history and a payment in flight
//   ?state=welcome        no wallet yet
//   ?state=new            a wallet just made, not backed up (can't receive)
//   ?state=empty          a wallet with nothing in it yet
//   ?state=deposit        a deposit to the VM confirming
//   ?state=signer-change  BTCVM's bridge reports new signers: moves paused
//   ?state=doge-change    DogecoinVM's bridge reports new signers
//   ?state=doge-unknown   a phrase wallet from before DOGE: its DOGE address
//                         is shown once with Windows Hello
//   ?state=rotation       the wallet followed a rotation
//   ?state=offline        the bridge can't be reached
//   ?state=untrusted      the bridge failed the wallet's checks
//   ?state=problem        the active wallet's vault can't be opened
//   ?state=busy           every notice at once
//   &coin=btc|doge        the coin shown first
//   &lang=es|en           the window's language
//
// window.calls lists every command the window sent, with its arguments.
(() => {
  const params = new URLSearchParams(location.search);
  const state = params.get('state') || 'wallet';
  let language = params.get('lang') === 'en' ? 'en' : 'es';
  let coin = params.get('coin') === 'doge' || state.startsWith('doge') ? 'doge' : 'btc';

  const A1 = 'bc1qxxwwe996pzvu2gqqhz4lsyr66acpwwy7ndmn5q';
  const A2 = 'bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4';
  const D1 = 'DBus3bamQjgJULBJtYXpEzDWQRwF5iwxgC';
  const D2 = 'DGx5w3j4u7MfwTJ1rUeVx8zGqDdVAhmwFd';
  const EXCHANGE = 'bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq';
  const DOGE_SHOP = 'DK3pSf2hT9rS1rJ4P1bW6m7XUjQnF4Y8Ns';
  const PEG = 'bc1qaunaxg4hu96ja6pvarfuph663lx96xau5dkhxcsg2ypve6n6wwlsvew3h6';
  const NEW_PEG = 'bc1q9vza2e8x573nczrlzms0wvx3gsqjx7vavgkx0l00zvwz8lqgvxnstjnldu';
  const DOGE_PEG = 'AAvNfukpAa4iTcRJPetxuxX8XxbC5gFqUM';
  const DOGE_NEW_PEG = 'A9zxR4X5k7mVqP2nJdFh3tWcYbGs8uLeKo';
  const tx = (c) => c.repeat(64).slice(0, 64);
  const now = Math.floor(Date.now() / 1000);

  // Each coin's chains and ticker, as the Rust side names them.
  const COINS = {
    btc: { ticker: 'BTC', l1: 'bitcoin', vm: 'btcvm', price: { EUR: 55000, USD: 60000 } },
    doge: { ticker: 'DOGE', l1: 'dogecoin', vm: 'dogecoinvm', price: { EUR: 0.21, USD: 0.23 } },
  };

  let active = 'main';
  let fiat = 'EUR';
  let checkBalances = false;
  let updatesOn = true;
  let check = null;
  let rotationSeen = false;
  let softwareSeen = false;
  let dogeLearned = state !== 'doge-unknown';
  const names = { main: '', '3fa9c2d1e0b7': 'Ahorro' };
  let book = [
    { name: 'Exchange', address: EXCHANGE, chain: 'bitcoin' },
    { name: 'Ana', address: 'bc1qrp33g0q5c5txsp9arysrx4k6zdkfs4nce4xj0gdcccefvpysxf3qccfmv3', chain: 'btcvm' },
    { name: 'Tienda DOGE', address: DOGE_SHOP, chain: 'dogecoin' },
  ];
  let wallets = state === 'welcome' ? [] : [
    { id: 'main', address: { btc: A1, doge: D1 }, needsBackup: state === 'new', hardware: state === 'busy' ? false : true },
    { id: '3fa9c2d1e0b7', address: { btc: A2, doge: D2 }, needsBackup: false, hardware: true },
  ];
  const empty = state === 'empty' || state === 'new';
  const zero = { confirmed: '0', pending: '0' };
  // Per wallet and coin: [its own chain, its VM].
  const balances = {
    main: {
      btc: empty ? [zero, zero] : [{ confirmed: '0.0002', pending: '0' }, { confirmed: '0.00015', pending: '0.00002' }],
      doge: empty ? [zero, zero] : [{ confirmed: '1520.5', pending: '0' }, { confirmed: '48.99', pending: '12' }],
    },
    '3fa9c2d1e0b7': { btc: [zero, { confirmed: '0.00031', pending: '0' }], doge: [{ confirmed: '300', pending: '0' }, zero] },
  };

  function bridge(c) {
    const server = c === 'doge' ? 'https://metaldoge.com' : 'https://metalbtc.com';
    if (state === 'offline') return { coin: c, server, connected: false, trusted: false, error: null, signerChange: null, paused: false, l1Syncing: false };
    const doge = c === 'doge';
    const b = doge
      ? {
        coin: c, server, connected: true, trusted: true, error: null, signerChange: null, pegAddress: DOGE_PEG,
        minDeposit: '1', maxDeposit: '100', minPegOut: '2', feeRate: 100, vmFee: '0.01', payoutFee: '0.0226',
        paused: false, solvent: true, locked: '1250.5', circulating: '1250.49', l1Syncing: false, rotation: null,
      }
      : {
        coin: c, server, connected: true, trusted: state !== 'untrusted', error: null, signerChange: null, pegAddress: PEG,
        minDeposit: '0.0001', maxDeposit: '0.001', minPegOut: '0.00005', feeRate: 3, vmFee: '0.0000001', payoutFee: '0.00000516',
        paused: false, solvent: true, locked: '0.0123', circulating: '0.0121', l1Syncing: false, rotation: null,
      };
    if (!doge && state === 'untrusted') b.error = { message: "the bridge's peg address doesn't follow from its signers", untrusted: true, canceled: false };
    if (!doge && (state === 'signer-change' || state === 'busy')) {
      b.signerChange = {
        coin: c, trustedPeg: PEG, reportedPeg: NEW_PEG, added: ['02aa…', '03bb…', '02cc…'], removed: ['03551e…', '03cb2d…', '034ee7…'],
        links: ['oldPegOnBitcoin', 'newPegOnBitcoin', 'rotationProcedure', 'btcvmDocs', 'btcvmExplorer', 'walletMaker'].map((source) => ({ source, url: 'https://example.org' })),
      };
    }
    if (doge && (state === 'doge-change' || state === 'busy')) {
      b.signerChange = {
        coin: c, trustedPeg: DOGE_PEG, reportedPeg: DOGE_NEW_PEG, added: ['02dd…'], removed: ['0212a0…'],
        links: ['oldPegOnDogecoin', 'newPegOnDogecoin', 'dogecoinvmSigners', 'dogecoinvmDocs', 'dogecoinvmExplorer', 'walletMaker'].map((source) => ({ source, url: 'https://example.org' })),
      };
    }
    if (!doge && (state === 'rotation' || state === 'busy') && !rotationSeen) b.rotation = { fromPeg: PEG, toPeg: NEW_PEG, chain: 'btcvm', txid: tx('cd') };
    return b;
  }

  /** A wallet's address for `c`: none for a phrase wallet's DOGE until it
   *  is learned. */
  const addressOf = (w, c) => (c === 'doge' && w.id === 'main' && !dogeLearned ? null : w.address[c]);
  const price = (c) => (fiat === 'none' ? null : COINS[c].price[fiat]);

  function view() {
    const C = COINS[coin];
    const list = wallets.map((w) => ({
      id: w.id, name: names[w.id] || '', active: w.id === active, needsBackup: w.needsBackup, hardware: w.hardware,
      address: w.needsBackup ? null : addressOf(w, coin),
      l1: balances[w.id][coin][0], vm: balances[w.id][coin][1],
    }));
    const w = wallets.find((x) => x.id === active);
    const shown = list.find((x) => x.active) || {};
    const unknown = !!w && !addressOf(w, coin);
    const known = !!w && !unknown && state !== 'offline';
    const history = empty || !known ? [] : coin === 'doge'
      ? [
        { chain: 'dogecoinvm', txid: tx('d1'), net: '12', confirmations: 0, time: null },
        { chain: 'dogecoin', txid: tx('d7'), net: '1500', confirmations: 30, time: now - 2 * 86400 },
        { chain: 'dogecoinvm', txid: tx('d3'), net: '-11.01', confirmations: 80, time: now - 4 * 86400 },
      ]
      : [
        { chain: 'btcvm', txid: tx('e1'), net: '0.00015', confirmations: 0, time: null },
        { chain: 'bitcoin', txid: tx('77'), net: '0.0002', confirmations: 12, time: now - 86400 },
        { chain: 'btcvm', txid: tx('3d'), net: '-0.00005001', confirmations: 40, time: now - 3 * 86400 },
        { chain: 'bitcoin', txid: tx('9a'), net: '-0.00030423', confirmations: 120, time: now - 9 * 86400 },
      ];
    const deposits = state === 'deposit' ? [coin === 'doge'
      ? { txid: tx('dd'), vout: 0, amount: '25', confirmations: 3, required: 6, status: 'confirming', reason: null, credited: null, creditTxid: null, refundTxid: null }
      : { txid: tx('de'), vout: 0, amount: '0.00019847', confirmations: 1, required: 2, status: 'confirming', reason: null, credited: null, creditTxid: null, refundTxid: null }] : [];
    const inFlight = (state === 'wallet' || state === 'busy') && known
      ? coin === 'doge'
        ? [{ txid: tx('da'), chain: 'dogecoin', kind: 'send', to: DOGE_SHOP, amount: 2000000000, change: 0, spent: [], time: now - 300 }]
        : [{ txid: tx('ab'), chain: 'bitcoin', kind: 'send', to: EXCHANGE, amount: 30000, change: 69577, spent: [], time: now - 600 }]
      : [];
    const withdrawals = state === 'deposit' && coin === 'doge'
      ? [{ coin: 'doge', txid: tx('wd'), to: D1, amount: 1000000000, status: 'pending', pays: null, paymentTxid: null, paymentConfirmations: 0, time: now - 900 }]
      : [];
    const coins = Object.keys(COINS).map((c) => {
      const b = bridge(c);
      const k = !!w && !!addressOf(w, c) && state !== 'offline';
      return {
        coin: c, l1: k ? balances[w.id][c][0] : null, vm: k ? balances[w.id][c][1] : null, price: price(c),
        attention: !b.connected || !!b.error || !!b.signerChange,
      };
    });
    return {
      version: '0.3.0', server: 'https://metalbtc.com', language, hasWallet: wallets.length > 0, wallets: list,
      walletId: wallets.length ? active : null, walletName: shown.name || '',
      vaultProblem: state === 'problem' ? "the wallet's vault is damaged: the vault doesn't open with this Windows Hello key" : null,
      coin, coins,
      address: shown.address || null, addressUnknown: unknown && state !== 'problem', receiveBlocked: !!shown.needsBackup, backupConfirmed: !shown.needsBackup,
      l1: known ? shown.l1 || null : null, vm: known ? shown.vm || null : null,
      l1Note: null, history, deposits, withdrawals, inFlight, addressBook: book, bridge: bridge(coin),
      connectionError: state === 'offline' ? "can't reach the bridge: io: timed out" : null,
      vmIncoming: state === 'deposit' ? (coin === 'doge' ? '24.99' : '0.00019837') : null,
      internalError: state === 'busy' ? 'called `Option::unwrap()` on a `None` value (app\\src\\wallet.rs:1234)' : null,
      fiatChoice: fiat, fiat: price(coin) == null ? null : { currency: fiat, price: price(coin) },
      hardware: shown.hardware ?? null, softwareKeyWarning: shown.hardware === false && !softwareSeen,
      backupCheck: check, checkBalances, l1Disagrees: state === 'busy' && coin === 'btc' ? '0.00018' : null,
      updatesOn, updatesPossible: true, updated: now,
    };
  }

  const release = state === 'busy' ? { version: '0.4.0', notes: { es: 'Mejoras de usabilidad y correcciones.', en: 'Usability improvements and fixes.' } } : null;
  const fail = (message, untrusted = false) => { throw { message, untrusted, canceled: false }; };
  const review = (r) => ({ id: Date.now(), walletName: names[active] || 'Principal', ticker: COINS[coin].ticker, lookalike: null, credited: null, confirmations: null, payoutFee: null, previousFee: null, ...r });
  const own = () => wallets.find((x) => x.id === active).address[coin];

  const handlers = {
    hello: () => view(),
    view: () => view(),
    refresh: () => view(),
    fee_options: () => ({ bridge: 3, fastest: 6, halfHour: 4, hour: 2, economy: 1, minimum: 1, max: 1000 }),
    set_coin: (a) => {
      if (!COINS[a.coin]) fail('unknown coin');
      coin = a.coin;
      return view();
    },
    learn_doge_address: () => { dogeLearned = true; return view(); },
    select_wallet: ({ id }) => { active = id; return view(); },
    rename_wallet: ({ id, name }) => { names[id] = name.trim(); return view(); },
    create_wallet: ({ name }) => {
      const id = Math.random().toString(16).slice(2, 14);
      wallets.push({ id, address: { btc: 'bc1q' + id.padEnd(38, 'q'), doge: 'D' + id.padEnd(33, 'x') }, needsBackup: true, hardware: true });
      balances[id] = { btc: [zero, zero], doge: [zero, zero] };
      names[id] = name || '';
      active = id;
      check = { kind: 'words', items: [[3, 3], [7, 7], [11, 11]] };
      return view();
    },
    import_from_clipboard: () => fail('copy your recovery phrase or your key (WIF) first, then press the button'),
    restore_from_clipboard: () => fail('copy your recovery phrase or your key (WIF) first, then press the button'),
    backup: () => { check = { kind: 'words', items: [[2, 2], [5, 5], [12, 12]] }; return view(); },
    verify_backup: ({ answers }) => {
      if (answers.some((a) => a.trim().length < 3)) fail("that doesn't match the backup you were shown: look at it again, or show it again");
      check = null;
      wallets = wallets.map((w) => (w.id === active ? { ...w, needsBackup: false } : w));
      return view();
    },
    cancel_backup_check: () => { check = null; return view(); },
    remove_wallet: () => view(),
    add_contact: ({ name, address, chain }) => {
      if (!name.trim()) fail('give the address a name');
      const doge = chain === 'dogecoin' || chain === 'dogecoinvm';
      if (!(doge ? /^[DA9][1-9A-HJ-NP-Za-km-z]{25,34}$/ : /^bc1[a-z0-9]{20,}$/).test(address.trim())) fail('not a valid address');
      book = [...book, { name: name.trim(), address: address.trim(), chain }];
      return view();
    },
    rename_contact: ({ address, chain, name }) => { book = book.map((c) => (c.address === address && c.chain === chain ? { ...c, name } : c)); return view(); },
    remove_contact: ({ address, chain }) => { book = book.filter((c) => !(c.address === address && c.chain === chain)); return view(); },
    max_amount: ({ action, feeRate }) => {
      if (coin === 'doge') return action === 'deposit' ? '100' : action === 'withdraw' ? '48.98' : '1520.4';
      return action === 'deposit' ? '0.000195' : feeRate ? (0.0002 - feeRate * 141 / 1e8).toFixed(8) : '0.00014999';
    },
    prepare_send: ({ chain, to, amount, feeRate }) => {
      if (!to.trim()) fail('not an address');
      if (!amount.trim()) fail(`enter an amount like ${coin === 'doge' ? '12.5' : '0.0025'}`);
      const doge = coin === 'doge';
      const known = to === wallets[1].address[coin] ? 'own' : book.some((c) => c.address === to && c.chain === chain) ? 'book' : book.some((c) => c.address === to) ? 'bookOtherChain' : 'new';
      return review({
        kind: 'send', chain, to,
        toKnown: known,
        toName: known === 'own' ? 'Ahorro' : (book.find((c) => c.address === to) || {}).name || null,
        lookalike: to.startsWith('bc1qar0s') && to !== EXCHANGE ? 'Exchange' : null,
        amount,
        fee: doge ? (chain === 'dogecoin' ? '0.00226' : '0.000226') : chain === 'bitcoin' ? '0.00000423' : '0.00000001',
        feeRate: chain === 'bitcoin' ? feeRate : null,
        total: doge ? '10.00226' : '0.00010423',
        outputs: [{ value: amount, address: to, role: 'pay', withdrawalTo: null }, { value: doge ? '1510.49774' : '0.00009577', address: own(), role: 'change', withdrawalTo: null }],
      });
    },
    prepare_deposit: ({ amount, feeRate }) => (coin === 'doge'
      ? review({
        kind: 'deposit', chain: 'dogecoin', to: '9xDepositAddressxxxxxxxxxxxxxxxxx', toKnown: 'deposit', toName: null,
        amount, fee: '0.00226', feeRate: null, total: '25.00226', credited: '24.99', confirmations: 6,
        outputs: [{ value: amount, address: '9xDepositAddressxxxxxxxxxxxxxxxxx', role: 'deposit', withdrawalTo: null }, { value: '1495.49774', address: D1, role: 'change', withdrawalTo: null }],
      })
      : review({
        kind: 'deposit', chain: 'bitcoin', to: 'bc1qdeposit', toKnown: 'deposit', toName: null,
        amount, fee: '0.00000600', feeRate, total: '0.0001006', credited: '0.00009990', confirmations: 2,
        outputs: [{ value: amount, address: 'bc1q8cz7v0k6mkxmt4g0j5mzuz3ux2xf8fzs8j0u9h9mfmg3e6fsyjhs2f6l9n', role: 'deposit', withdrawalTo: null }, { value: '0.0000994', address: A1, role: 'change', withdrawalTo: null }],
      })),
    prepare_withdrawal: ({ to, amount }) => {
      const doge = coin === 'doge';
      const mine = to === own();
      return review({
        kind: 'withdraw', chain: COINS[coin].vm, to, toKnown: mine ? 'own' : 'new', toName: mine ? 'Principal' : null,
        amount, fee: doge ? '0.000253' : '0.00000001', total: amount, payoutFee: doge ? '0.0226' : '0.00000516',
        outputs: [{ value: amount, address: doge ? DOGE_PEG : PEG, role: 'reserve', withdrawalTo: null }, { value: '0', address: null, role: 'tag', withdrawalTo: to }, { value: doge ? '38.989747' : '0.0000999', address: own(), role: 'change', withdrawalTo: null }],
      });
    },
    prepare_bump: ({ feeRate }) => review({
      kind: 'bump', chain: 'bitcoin', ticker: 'BTC', to: EXCHANGE, toKnown: 'book', toName: 'Exchange', amount: '0.0003', fee: '0.0000141', feeRate,
      previousFee: '0.00000282', total: '0.0003141',
      outputs: [{ value: '0.0003', address: EXCHANGE, role: 'pay', withdrawalTo: null }, { value: '0.0000859', address: A1, role: 'change', withdrawalTo: null }],
    }),
    confirm: () => ({ txid: tx('f0'), chain: COINS[coin].l1 }),
    cancel: () => null,
    set_server: () => view(),
    set_language: (a) => { language = a.language; return view(); },
    set_fiat: ({ currency }) => { fiat = currency; return view(); },
    set_check_balances: ({ on }) => { checkBalances = on; return view(); },
    set_updates: ({ on }) => { updatesOn = on; return view(); },
    check_update: () => release,
    update_info: () => release,
    install_update: () => fail('the download failed: this is a preview'),
    rotation_seen: () => { rotationSeen = true; return view(); },
    check_rotation: () => view(),
    software_key_seen: () => { softwareSeen = true; return view(); },
    copy_address: () => null,
    open_link: () => null,
    open_logs: () => null,
    export_history: () => true,
    receive_qr: () => {
      const width = 29;
      let modules = '';
      for (let y = 0; y < width; y++) {
        for (let x = 0; x < width; x++) {
          const finder = (a, b) => a < 7 && b < 7 && (a === 0 || a === 6 || b === 0 || b === 6 || (a > 1 && a < 5 && b > 1 && b < 5));
          const dark = finder(x, y) || finder(width - 1 - x, y) || finder(x, width - 1 - y) || (!(x < 8 && y < 8) && ((x * 7 + y * 13 + x * y) % 3 === 0));
          modules += dark ? '1' : '0';
        }
      }
      return { width, modules, address: own() };
    },
    about_info: () => ({ creator: 'madblocks', role: 'XPR Network BP', producer: 'madblocks', metalNodeId: 'NodeID-B1hsNPKgi6C89AFybyPFPvDQC2gHxMv7H', version: '0.3.0' }),
  };

  window.calls = [];
  const listeners = {};
  window.__TAURI__ = {
    core: {
      invoke: async (cmd, args = {}) => {
        window.calls.push([cmd, args]);
        await new Promise((r) => setTimeout(r, 60));
        const f = handlers[cmd];
        if (!f) throw { message: `preview: no command ${cmd}`, untrusted: false, canceled: false };
        return f(args);
      },
    },
    event: {
      listen: async (name, cb) => {
        listeners[name] = cb;
        return () => { delete listeners[name]; };
      },
    },
  };
  // A payment arriving, to see the notice: call window.preview.notice().
  window.preview = {
    notice: (amount = '0.0005', chain = 'btcvm', confirmed = false) => listeners.notice && listeners.notice({ payload: { chain, amount, confirmed } }),
  };
})();
