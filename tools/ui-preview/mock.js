// A stand-in for the wallet's Rust side, to look at the window in a plain
// browser: tools/ui-preview/serve.py injects it before app.js. Nothing here
// is real: no keys, no network. Pick what to show in the address bar:
//
//   ?state=wallet         a wallet in use, with history and a payment in flight
//   ?state=welcome        no wallet yet
//   ?state=new            a wallet just made, not backed up (can't receive)
//   ?state=empty          a wallet with nothing in it yet
//   ?state=deposit        a deposit to BTCVM confirming
//   ?state=signer-change  the bridge reports new signers: moves paused
//   ?state=rotation       the wallet followed a rotation
//   ?state=offline        the bridge can't be reached
//   ?state=untrusted      the bridge failed the wallet's checks
//   ?state=problem        the active wallet's vault can't be opened
//   ?state=busy           every notice at once
//   &lang=es|en           the window's language
//
// window.calls lists every command the window sent, with its arguments.
(() => {
  const params = new URLSearchParams(location.search);
  const state = params.get('state') || 'wallet';
  let language = params.get('lang') === 'en' ? 'en' : 'es';

  const A1 = 'bc1qxxwwe996pzvu2gqqhz4lsyr66acpwwy7ndmn5q';
  const A2 = 'bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4';
  const EXCHANGE = 'bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq';
  const PEG = 'bc1qaunaxg4hu96ja6pvarfuph663lx96xau5dkhxcsg2ypve6n6wwlsvew3h6';
  const NEW_PEG = 'bc1q9vza2e8x573nczrlzms0wvx3gsqjx7vavgkx0l00zvwz8lqgvxnstjnldu';
  const tx = (c) => c.repeat(64).slice(0, 64);
  const now = Math.floor(Date.now() / 1000);

  let active = 'main';
  let fiat = 'EUR';
  let checkBalances = false;
  let updatesOn = true;
  let check = null;
  let rotationSeen = false;
  let softwareSeen = false;
  const names = { main: '', '3fa9c2d1e0b7': 'Ahorro' };
  let book = [
    { name: 'Exchange', address: EXCHANGE, chain: 'bitcoin' },
    { name: 'Ana', address: 'bc1qrp33g0q5c5txsp9arysrx4k6zdkfs4nce4xj0gdcccefvpysxf3qccfmv3', chain: 'btcvm' },
  ];
  let wallets = state === 'welcome' ? [] : [
    { id: 'main', address: A1, needsBackup: state === 'new', hardware: state === 'busy' ? false : true },
    { id: '3fa9c2d1e0b7', address: A2, needsBackup: false, hardware: true },
  ];
  const empty = state === 'empty' || state === 'new';
  const balances = {
    main: empty ? [{ confirmed: '0', pending: '0' }, { confirmed: '0', pending: '0' }] : [{ confirmed: '0.0002', pending: '0' }, { confirmed: '0.00015', pending: '0.00002' }],
    '3fa9c2d1e0b7': [{ confirmed: '0', pending: '0' }, { confirmed: '0.00031', pending: '0' }],
  };

  function bridge() {
    if (state === 'offline') return { connected: false, trusted: false, error: null, signerChange: null, paused: false, bitcoinSyncing: false };
    const b = {
      connected: true, trusted: state !== 'untrusted', error: null, signerChange: null, pegAddress: PEG,
      minDeposit: '0.0001', maxDeposit: '0.001', minPegOut: '0.00005', feeRate: 3, vmFee: '0.0000001', payoutFee: '0.00000516',
      paused: false, solvent: true, locked: '0.0123', circulating: '0.0121', bitcoinSyncing: false, rotation: null,
    };
    if (state === 'untrusted') b.error = { message: "the bridge's peg address doesn't follow from its signers", untrusted: true, canceled: false };
    if (state === 'signer-change' || state === 'busy') {
      b.signerChange = {
        trustedPeg: PEG, reportedPeg: NEW_PEG, added: ['02aa…', '03bb…', '02cc…'], removed: ['03551e…', '03cb2d…', '034ee7…'],
        links: ['oldPegOnBitcoin', 'newPegOnBitcoin', 'rotationProcedure', 'btcvmDocs', 'btcvmExplorer', 'walletMaker'].map((source) => ({ source, url: 'https://example.org' })),
      };
    }
    if ((state === 'rotation' || state === 'busy') && !rotationSeen) b.rotation = { fromPeg: PEG, toPeg: NEW_PEG, chain: 'btcvm', txid: tx('cd') };
    return b;
  }

  function view() {
    const list = wallets.map((w) => ({
      ...w, name: names[w.id] || '', active: w.id === active, address: w.needsBackup ? null : w.address,
      bitcoin: balances[w.id][0], btcvm: balances[w.id][1],
    }));
    const w = list.find((x) => x.active) || {};
    const history = empty ? [] : [
      { chain: 'btcvm', txid: tx('e1'), net: '0.00015', confirmations: 0, time: null },
      { chain: 'bitcoin', txid: tx('77'), net: '0.0002', confirmations: 12, time: now - 86400 },
      { chain: 'btcvm', txid: tx('3d'), net: '-0.00005001', confirmations: 40, time: now - 3 * 86400 },
      { chain: 'bitcoin', txid: tx('9a'), net: '-0.00030423', confirmations: 120, time: now - 9 * 86400 },
    ];
    const deposits = state === 'deposit' ? [{ txid: tx('de'), vout: 0, amount: '0.00019847', confirmations: 1, required: 2, status: 'confirming', reason: null, credited: null, creditTxid: null, refundTxid: null }] : [];
    const inFlight = state === 'wallet' || state === 'busy'
      ? [{ txid: tx('ab'), chain: 'bitcoin', kind: 'send', to: EXCHANGE, amount: 30000, change: 69577, spent: [], time: now - 600 }]
      : [];
    return {
      version: '0.2.0', server: 'https://metalbtc.com', language, hasWallet: wallets.length > 0, wallets: list,
      walletId: wallets.length ? active : null, walletName: w.name || '',
      vaultProblem: state === 'problem' ? "the wallet's vault is damaged: the vault doesn't open with this Windows Hello key" : null,
      address: w.address || null, receiveBlocked: !!w.needsBackup, backupConfirmed: !w.needsBackup,
      bitcoin: state === 'offline' ? null : w.bitcoin || null, btcvm: state === 'offline' ? null : w.btcvm || null,
      bitcoinNote: null, history, deposits, withdrawals: [], inFlight, addressBook: book, bridge: bridge(),
      connectionError: state === 'offline' ? "can't reach the bridge: io: timed out" : null,
      btcvmIncoming: state === 'deposit' ? '0.00019837' : null,
      internalError: state === 'busy' ? 'called `Option::unwrap()` on a `None` value (app\\src\\wallet.rs:1234)' : null,
      fiatChoice: fiat, fiat: fiat === 'none' ? null : { currency: fiat, price: fiat === 'EUR' ? 55000 : 60000 },
      hardware: w.hardware ?? null, softwareKeyWarning: w.hardware === false && !softwareSeen,
      backupCheck: check, checkBalances, bitcoinDisagrees: state === 'busy' ? '0.00018' : null,
      updatesOn, updatesPossible: true, updated: now,
    };
  }

  const release = state === 'busy' ? { version: '0.3.0', notes: { es: 'Mejoras de usabilidad y correcciones.', en: 'Usability improvements and fixes.' } } : null;
  const fail = (message, untrusted = false) => { throw { message, untrusted, canceled: false }; };
  const review = (r) => ({ id: Date.now(), walletName: names[active] || 'Principal', lookalike: null, credited: null, confirmations: null, payoutFee: null, previousFee: null, ...r });

  const handlers = {
    hello: () => view(),
    view: () => view(),
    refresh: () => view(),
    fee_options: () => ({ bridge: 3, fastest: 6, halfHour: 4, hour: 2, economy: 1, minimum: 1, max: 1000 }),
    select_wallet: ({ id }) => { active = id; return view(); },
    rename_wallet: ({ id, name }) => { names[id] = name.trim(); return view(); },
    create_wallet: ({ name }) => {
      const id = Math.random().toString(16).slice(2, 14);
      wallets.push({ id, address: 'bc1q' + id.padEnd(38, 'q'), needsBackup: true, hardware: true });
      balances[id] = [{ confirmed: '0', pending: '0' }, { confirmed: '0', pending: '0' }];
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
      if (!/^bc1[a-z0-9]{20,}$/.test(address.trim())) fail('not a valid address');
      book = [...book, { name: name.trim(), address: address.trim(), chain }];
      return view();
    },
    rename_contact: ({ address, chain, name }) => { book = book.map((c) => (c.address === address && c.chain === chain ? { ...c, name } : c)); return view(); },
    remove_contact: ({ address, chain }) => { book = book.filter((c) => !(c.address === address && c.chain === chain)); return view(); },
    max_amount: ({ action, feeRate }) => (action === 'deposit' ? '0.000195' : feeRate ? (0.0002 - feeRate * 141 / 1e8).toFixed(8) : '0.00014999'),
    prepare_send: ({ chain, to, amount, feeRate }) => {
      if (!to.trim()) fail('not an address');
      if (!amount.trim()) fail('enter an amount like 0.0025');
      return review({
        kind: 'send', chain, to,
        toKnown: to === A2 ? 'own' : book.some((c) => c.address === to && c.chain === chain) ? 'book' : book.some((c) => c.address === to) ? 'bookOtherChain' : 'new',
        toName: to === A2 ? 'Ahorro' : (book.find((c) => c.address === to) || {}).name || null,
        lookalike: to.startsWith('bc1qar0s') && to !== EXCHANGE ? 'Exchange' : null,
        amount, fee: chain === 'bitcoin' ? '0.00000423' : '0.00000001', feeRate: chain === 'bitcoin' ? feeRate : null, total: '0.00010423',
        outputs: [{ value: amount, address: to, role: 'pay', withdrawalTo: null }, { value: '0.00009577', address: A1, role: 'change', withdrawalTo: null }],
      });
    },
    prepare_deposit: ({ amount, feeRate }) => review({
      kind: 'deposit', chain: 'bitcoin', to: 'bc1qdeposit', toKnown: 'deposit', toName: null,
      amount, fee: '0.00000600', feeRate, total: '0.0001006', credited: '0.00009990', confirmations: 2,
      outputs: [{ value: amount, address: 'bc1q8cz7v0k6mkxmt4g0j5mzuz3ux2xf8fzs8j0u9h9mfmg3e6fsyjhs2f6l9n', role: 'deposit', withdrawalTo: null }, { value: '0.0000994', address: A1, role: 'change', withdrawalTo: null }],
    }),
    prepare_withdrawal: ({ to, amount }) => review({
      kind: 'withdraw', chain: 'btcvm', to, toKnown: to === A1 ? 'own' : 'new', toName: to === A1 ? 'Principal' : null,
      amount, fee: '0.00000001', total: amount, payoutFee: '0.00000516',
      outputs: [{ value: amount, address: PEG, role: 'reserve', withdrawalTo: null }, { value: '0', address: null, role: 'tag', withdrawalTo: to }, { value: '0.0000999', address: A1, role: 'change', withdrawalTo: null }],
    }),
    prepare_bump: ({ feeRate }) => review({
      kind: 'bump', chain: 'bitcoin', to: EXCHANGE, toKnown: 'book', toName: 'Exchange', amount: '0.0003', fee: '0.0000141', feeRate,
      previousFee: '0.00000282', total: '0.0003141',
      outputs: [{ value: '0.0003', address: EXCHANGE, role: 'pay', withdrawalTo: null }, { value: '0.0000859', address: A1, role: 'change', withdrawalTo: null }],
    }),
    confirm: () => ({ txid: tx('f0'), chain: 'bitcoin' }),
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
      return { width, modules, address: A1 };
    },
    about_info: () => ({ creator: 'madblocks', role: 'XPR Network BP', producer: 'madblocks', metalNodeId: 'NodeID-B1hsNPKgi6C89AFybyPFPvDQC2gHxMv7H', version: '0.2.0' }),
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
