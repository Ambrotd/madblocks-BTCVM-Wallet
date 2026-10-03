// The window of madblocks BTCVM Wallet. It shows what the Rust side sends
// and asks it to act. Keys never come here, and nothing from the bridge is
// ever inserted as HTML: every value goes in as text.

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const STRINGS = {
  es: {
    byline: 'por madblocks · BP de XPR Network y validador de Metal',
    about: 'Acerca de', settings: 'Ajustes', connected: 'Conectado', offline: 'Sin conexión',
    welcomeTitle: 'Tu wallet de Bitcoin y BTCVM',
    welcomeText: 'Una clave, la misma dirección en Bitcoin y en BTCVM, y Windows Hello para cada pago. La clave se crea en este PC y se guarda cifrada con una clave de Windows Hello que vive en el TPM.',
    create: 'Crear una wallet nueva', importClip: 'Importar mi clave desde el portapapeles',
    importHint: 'Para importar, copia tu clave (WIF, empieza por K o L) y pulsa el botón: se lee aquí y el portapapeles se borra.',
    helloTwice: 'Al crear o importar la wallet, Windows Hello te pedirá el PIN dos veces: una para crear su clave en el TPM y otra para cifrar tu clave con ella. Después, una vez por cada pago.',
    yourAddress: 'Tu dirección', sameAddress: 'La misma en Bitcoin y en BTCVM.', copy: 'Copiar', copied: 'Dirección copiada',
    onBitcoin: 'Ver en Bitcoin', onBtcvm: 'Ver en BTCVM',
    blockedReceive: 'Haz la copia de seguridad de tu clave antes de recibir: este PC tiene la única copia.',
    bitcoin: 'Bitcoin', btcvm: 'BTCVM', pending: 'pendiente', loading: 'cargando…',
    watching: 'Empezando a vigilar tu dirección de Bitcoin…', syncing: 'Se mostrará cuando el nodo de Bitcoin del puente se ponga al día.',
    unavailable: 'Este puente no sirve saldos de Bitcoin.',
    send: 'Enviar', deposit: 'Pasar a BTCVM', withdraw: 'Retirar a Bitcoin',
    network: 'Red', to: 'Destino', amount: 'Cantidad (BTC)', review: 'Revisar',
    max: 'Máx.', maxHint: 'Todo lo que puedes mover ahora, descontada la comisión (y sin pasar del máximo del puente en un depósito)',
    sendHint: 'Elige la red cada vez: la misma dirección existe en las dos, pero las monedas no.',
    depositHint: 'Paga desde tu saldo de Bitcoin a tu dirección de depósito personal, calculada aquí con las claves fijadas de los firmantes y comprobada con el puente. BTCVM la acredita tras las confirmaciones que pide el puente (menos para cantidades pequeñas).',
    limits: 'Mínimo {min} BTC', maxCap: 'máximo {max} BTC (límite de la alfa)', vmFeeNote: 'el puente se queda {fee} BTC',
    withdrawHint: 'Paga la reserva del puente en BTCVM con una etiqueta que nombra tu dirección de Bitcoin. El puente paga allí la cantidad menos la comisión de Bitcoin (hoy unos {fee} BTC).',
    useMine: 'Usar mi dirección', btcAddress: 'Dirección de Bitcoin',
    paused: 'En pausa: han cambiado los firmantes del puente (ver aviso arriba).',
    inFlight: 'En curso', nothingInFlight: 'Nada en curso.',
    history: 'Historial', noHistory: 'Todavía no hay movimientos.', view: 'ver',
    unconfirmed: 'sin confirmar', confs: '{n} conf.',
    bridge: 'El puente', pegPinned: 'Dirección del peg (fijada en la wallet)', audit: 'Auditoría: {locked} BTC bloqueados para {circ} BTC en circulación',
    solvent: 'cubierto', notSolvent: 'NO cubierto', feeRate: 'Comisión de Bitcoin estimada: {rate} sat/vB',
    reviewTitle: { send: 'Enviar en {chain}', deposit: 'Pasar a BTCVM', withdraw: 'Retirar a Bitcoin' },
    role: { pay: 'Pago a', deposit: 'Tu dirección de depósito (comprobada con los firmantes fijados)', reserve: 'Reserva del puente (fijada en la wallet)', change: 'Cambio: vuelve a ti', tag: 'Etiqueta: el puente paga a', other: 'Otra salida' },
    fee: 'Comisión de red', total: 'Sale de tu wallet', credited: 'BTCVM acreditará', afterConf: 'tras {n} confirmaciones',
    payoutFee: 'Comisión de Bitcoin del pago, descontada de lo que recibes (aprox.)',
    confirm: 'Firmar con Windows Hello', cancel: 'Cancelar', waitingHello: 'Esperando a Windows Hello…', working: 'Preparando…',
    sent: 'Enviado. Id: {txid}', helloCanceled: 'Windows Hello cancelado: no se ha firmado nada.',
    untrustedPrefix: 'Comprobación fallida, posible ataque: ',
    kinds: { send: 'Envío', deposit: 'Depósito', withdraw: 'Retirada' },
    depositStatus: { confirming: 'Confirmando {c}/{r}', waiting_for_capacity: 'Esperando capacidad del puente', crediting: 'Acreditando', credited: 'Acreditado {amt} BTC', held: 'Retenido', refunded: 'Devuelto' },
    withdrawalStatus: { sending: 'Enviando', pending: 'Pendiente del pago en Bitcoin', paid: 'Pagada: {pays} BTC', unknown: 'Desconocida' },
    paidConfs: '({n} conf. en Bitcoin)', depositLabel: 'Depósito', withdrawalLabel: 'Retirada a {to}',
    bannerVault: 'No se puede abrir la bóveda de esta wallet', restoreClip: 'Restaurar desde la copia (portapapeles)',
    restoreHint: 'Copia tu clave de respaldo (WIF) y pulsa el botón. Tiene que ser la clave de esta wallet.',
    bannerUntrusted: 'El puente no ha pasado las comprobaciones de la wallet', bannerUntrustedText: 'No se firmará nada con este puente. Puede ser un error del servidor o un ataque.',
    bannerOffline: 'No se puede conectar con el puente', bannerPaused: 'Los operadores han pausado el puente.',
    bannerInsolvent: 'La auditoría del puente indica que el peg no está totalmente cubierto. No muevas fondos entre cadenas.',
    bannerBackup: 'Haz la copia de seguridad de tu clave', bannerBackupText: 'Hasta entonces no se muestra tu dirección: este PC tiene la única copia de la clave.', backupNow: 'Hacer copia (Windows Hello)',
    changeTitle: 'Han cambiado los firmantes del puente',
    tradeoff: 'La contrapartida: si cambian los firmantes del puente, los depósitos y las retiradas se pausan. Una rotación planificada por los operadores de BTCVM se ve igual que un servidor secuestrado, y la wallet no puede distinguirlos por sí sola. Por eso deja de mover fondos entre cadenas hasta que se actualice con el nuevo conjunto, te avisa y te dice dónde comprobarlo. Los envíos siguen funcionando. Es intencionado.',
    oldPeg: 'Peg fijado en la wallet', newPeg: 'Peg que informa ahora el puente', keysChanged: '{a} claves nuevas, {r} retiradas',
    checkIt: 'Dónde comprobarlo',
    sources: { oldPegOnBitcoin: 'El peg antiguo en un explorador de Bitcoin: tras una rotación real, sus BTC se han movido al nuevo (solo los firmantes antiguos podían hacerlo)', newPegOnBitcoin: 'El peg nuevo en un explorador de Bitcoin', rotationProcedure: 'Cómo rotan los firmantes los operadores de BTCVM', btcvmDocs: 'Documentación de BTCVM (metalbtc.com)', btcvmExplorer: 'Explorador de BTCVM (metalbtc.com)', walletMaker: 'madblocks publica cada conjunto de firmantes verificado con las actualizaciones' },
    server: 'Servidor del puente', save: 'Guardar', reset: 'Por defecto', language: 'Idioma',
    backupKey: 'Ver mi clave para la copia (Windows Hello)', removeWallet: 'Quitar la wallet de este PC',
    security: 'Seguridad',
    securityText: 'La clave se descifra solo para firmar, tras Windows Hello, y nunca llega a esta ventana. Las direcciones de depósito y la reserva del puente se calculan aquí a partir de los firmantes fijados en la wallet; el servidor no puede redirigir un pago. Cada moneda se comprueba contra la transacción que la creó y las comisiones tienen tope.',
    aboutTitle: 'Acerca de',
    aboutText: 'madblocks BTCVM Wallet la ha creado madblocks, block producer de XPR Network y validador de Metal Blockchain. Es gratuita; si te resulta útil, apoya a madblocks:',
    vote: 'Votar a madblocks en XPR Network', delegate: 'Delegar en el validador de Metal ({node})', website: 'madblocks.tech', xLink: 'Seguir a @madblocksbp en X',
    disclaimer: 'Wallet independiente: no la ha hecho ni la avala Metallicus. Usa la API pública del puente de BTCVM. Software sin auditar y BTCVM está en alfa: usa cantidades pequeñas.',
    license: 'Licencia MIT. Versión {v}.', close: 'Cerrar',
  },
  en: {
    byline: 'by madblocks · XPR Network BP and Metal validator',
    about: 'About', settings: 'Settings', connected: 'Connected', offline: 'Offline',
    welcomeTitle: 'Your Bitcoin and BTCVM wallet',
    welcomeText: 'One key, the same address on Bitcoin and BTCVM, and Windows Hello for every payment. The key is made on this PC and kept encrypted to a Windows Hello key that lives in the TPM.',
    create: 'Create a new wallet', importClip: 'Import my key from the clipboard',
    importHint: 'To import, copy your key (a WIF starting with K or L) and press the button: it is read here and the clipboard is cleared.',
    helloTwice: 'When you create or import the wallet, Windows Hello asks for your PIN twice: once to make its key in the TPM and once to encrypt your key with it. After that, once per payment.',
    yourAddress: 'Your address', sameAddress: 'The same on Bitcoin and on BTCVM.', copy: 'Copy', copied: 'Address copied',
    onBitcoin: 'View on Bitcoin', onBtcvm: 'View on BTCVM',
    blockedReceive: 'Back up your key before receiving: this PC holds the only copy.',
    bitcoin: 'Bitcoin', btcvm: 'BTCVM', pending: 'pending', loading: 'loading…',
    watching: 'Starting to watch your Bitcoin address…', syncing: 'Shows once the bridge\'s Bitcoin node has caught up.',
    unavailable: 'This bridge doesn\'t serve Bitcoin balances.',
    send: 'Send', deposit: 'Move to BTCVM', withdraw: 'Withdraw to Bitcoin',
    network: 'Network', to: 'To', amount: 'Amount (BTC)', review: 'Review',
    max: 'Max', maxHint: 'All you can move now, after the fee (and no more than the bridge accepts, for a deposit)',
    sendHint: 'Choose the network each time: the same address exists on both, the coins don\'t.',
    depositHint: 'Pays from your Bitcoin balance to your personal deposit address, computed here from the signers\' pinned keys and checked against the bridge. BTCVM credits it after the confirmations the bridge asks for (fewer for small amounts).',
    limits: 'At least {min} BTC', maxCap: 'at most {max} BTC (alpha cap)', vmFeeNote: 'the bridge keeps {fee} BTC',
    withdrawHint: 'Pays the bridge\'s reserve on BTCVM with a tag naming your Bitcoin address. The bridge pays the amount there, less Bitcoin\'s fee (about {fee} BTC today).',
    useMine: 'Use my address', btcAddress: 'Bitcoin address',
    paused: 'Paused: the bridge\'s signers have changed (see the warning above).',
    inFlight: 'In flight', nothingInFlight: 'Nothing in flight.',
    history: 'History', noHistory: 'No activity yet.', view: 'view',
    unconfirmed: 'unconfirmed', confs: '{n} conf.',
    bridge: 'The bridge', pegPinned: 'Peg address (pinned in the wallet)', audit: 'Audit: {locked} BTC locked for {circ} BTC circulating',
    solvent: 'backed', notSolvent: 'NOT backed', feeRate: 'Bitcoin fee estimate: {rate} sat/vB',
    reviewTitle: { send: 'Send on {chain}', deposit: 'Move to BTCVM', withdraw: 'Withdraw to Bitcoin' },
    role: { pay: 'Pays', deposit: 'Your deposit address (checked against the pinned signers)', reserve: 'The bridge\'s reserve (pinned in the wallet)', change: 'Change: back to you', tag: 'Tag: the bridge pays', other: 'Other output' },
    fee: 'Network fee', total: 'Leaves your wallet', credited: 'BTCVM will credit', afterConf: 'after {n} confirmations',
    payoutFee: 'Bitcoin fee of the payout, taken from what you receive (about)',
    confirm: 'Sign with Windows Hello', cancel: 'Cancel', waitingHello: 'Waiting for Windows Hello…', working: 'Preparing…',
    sent: 'Sent. Id: {txid}', helloCanceled: 'Windows Hello canceled: nothing was signed.',
    untrustedPrefix: 'Check failed, possibly an attack: ',
    kinds: { send: 'Payment', deposit: 'Deposit', withdraw: 'Withdrawal' },
    depositStatus: { confirming: 'Confirming {c}/{r}', waiting_for_capacity: 'Waiting for the bridge\'s capacity', crediting: 'Crediting', credited: 'Credited {amt} BTC', held: 'Held', refunded: 'Refunded' },
    withdrawalStatus: { sending: 'Sending', pending: 'Waiting for the Bitcoin payout', paid: 'Paid: {pays} BTC', unknown: 'Unknown' },
    paidConfs: '({n} conf. on Bitcoin)', depositLabel: 'Deposit', withdrawalLabel: 'Withdrawal to {to}',
    bannerVault: 'This wallet\'s vault can\'t be opened', restoreClip: 'Restore from the backup (clipboard)',
    restoreHint: 'Copy your backup key (WIF) and press the button. It must be this wallet\'s key.',
    bannerUntrusted: 'The bridge failed the wallet\'s checks', bannerUntrustedText: 'Nothing will be signed with this bridge. It may be a server fault or an attack.',
    bannerOffline: 'Can\'t reach the bridge', bannerPaused: 'The operators have paused the bridge.',
    bannerInsolvent: 'The bridge\'s audit shows the peg isn\'t fully backed. Don\'t move coins between the chains.',
    bannerBackup: 'Back up your key', bannerBackupText: 'Until then your address isn\'t shown: this PC holds the only copy of the key.', backupNow: 'Back up now (Windows Hello)',
    changeTitle: 'The bridge\'s signers have changed',
    tradeoff: 'The trade-off: if the bridge\'s signers change, deposits and withdrawals pause. A planned rotation by BTCVM\'s operators looks the same as a hijacked server, and the wallet can\'t tell them apart on its own. So it stops moving coins between the chains until it\'s updated with the new set, warns you, and shows where to check. Sends keep working. This is deliberate.',
    oldPeg: 'Peg pinned in the wallet', newPeg: 'Peg the bridge reports now', keysChanged: '{a} keys added, {r} removed',
    checkIt: 'Where to check it',
    sources: { oldPegOnBitcoin: 'The old peg on a Bitcoin explorer: after a real rotation its BTC have moved to the new one (only the old signers could do that)', newPegOnBitcoin: 'The new peg on a Bitcoin explorer', rotationProcedure: 'How BTCVM\'s operators rotate the signers', btcvmDocs: 'BTCVM\'s documentation (metalbtc.com)', btcvmExplorer: 'BTCVM\'s explorer (metalbtc.com)', walletMaker: 'madblocks publish each verified signer set with the wallet\'s updates' },
    server: 'Bridge server', save: 'Save', reset: 'Default', language: 'Language',
    backupKey: 'Show my key for backup (Windows Hello)', removeWallet: 'Remove the wallet from this PC',
    security: 'Security',
    securityText: 'The key is decrypted only to sign, after Windows Hello, and never reaches this window. Deposit addresses and the bridge\'s reserve are computed here from the signers pinned in the wallet; the server can\'t redirect a payment. Every coin is checked against the transaction that created it, and fees are capped.',
    aboutTitle: 'About',
    aboutText: 'madblocks BTCVM Wallet is made by madblocks, XPR Network block producer and Metal Blockchain validator. It\'s free; if it\'s useful to you, support madblocks:',
    vote: 'Vote for madblocks on XPR Network', delegate: 'Delegate to the Metal validator ({node})', website: 'madblocks.tech', xLink: 'Follow @madblocksbp on X',
    disclaimer: 'An independent wallet: not made or endorsed by Metallicus. It uses BTCVM\'s public bridge API. Unaudited software, and BTCVM is in alpha: keep amounts small.',
    license: 'MIT license. Version {v}.', close: 'Close',
  },
};

let lang = 'en';
let view = null;
let aboutInfo = null;
let mode = null;
let tab = 'send';
let busy = false;

function t(key, vars) {
  let s = key.split('.').reduce((o, k) => (o == null ? o : o[k]), STRINGS[lang]);
  if (s == null) s = key.split('.').reduce((o, k) => (o == null ? o : o[k]), STRINGS.en) ?? key;
  if (vars) for (const [k, v] of Object.entries(vars)) s = s.replaceAll(`{${k}}`, String(v));
  return s;
}

/** Builds an element. Strings become text nodes, never HTML. */
function h(tag, attrs, ...children) {
  const el = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs || {})) {
    if (v == null || v === false) continue;
    if (k === 'class') el.className = v;
    else if (k.startsWith('on')) el.addEventListener(k.slice(2), v);
    else el.setAttribute(k, v === true ? '' : v);
  }
  for (const c of children.flat()) {
    if (c == null || c === false) continue;
    el.append(c instanceof Node ? c : document.createTextNode(String(c)));
  }
  return el;
}

const $ = (id) => document.getElementById(id);
const short = (s) => (s && s.length > 20 ? `${s.slice(0, 10)}…${s.slice(-8)}` : s);
const when = (secs) => (secs ? new Date(secs * 1000).toLocaleString(lang) : '');

function toast(message, bad = false) {
  const el = $('toast');
  el.textContent = message;
  el.className = bad ? 'bad' : '';
  el.hidden = false;
  clearTimeout(toast.timer);
  toast.timer = setTimeout(() => { el.hidden = true; }, bad ? 9000 : 5000);
}

function showError(e) {
  if (e && e.canceled) return toast(t('helloCanceled'));
  const message = (e && e.message) || String(e);
  toast((e && e.untrusted ? t('untrustedPrefix') : '') + message, true);
}

async function act(fn) {
  if (busy) return;
  busy = true;
  document.body.classList.add('busy');
  try {
    const result = await fn();
    if (result && result.hasWallet !== undefined) apply(result);
    return result;
  } catch (e) {
    showError(e);
  } finally {
    busy = false;
    document.body.classList.remove('busy');
  }
}

const link = (kind, arg, label) => h('button', { class: 'link', type: 'button', onclick: () => invoke('open_link', { kind, arg: arg || '' }).catch(showError) }, label);

// --- the header and banners ------------------------------------------------------

function renderChrome() {
  document.documentElement.lang = lang;
  for (const el of document.querySelectorAll('[data-t]')) el.textContent = t(el.dataset.t);
  $('lang').textContent = lang === 'es' ? 'EN' : 'ES';
  const net = $('net');
  const online = view && view.bridge.connected && !view.connectionError;
  net.textContent = online ? t('connected') : t('offline');
  net.className = online ? 'pill' : 'pill off';
}

function banner(bad, title, ...body) {
  return h('div', { class: bad ? 'banner bad' : 'banner' }, h('h3', {}, title), ...body);
}

function renderBanners() {
  const out = [];
  const b = view.bridge;
  if (view.vaultProblem) {
    out.push(banner(true, t('bannerVault'), h('p', {}, view.vaultProblem), h('p', {}, t('restoreHint')),
      h('div', { class: 'row' }, h('button', { type: 'button', onclick: () => act(() => invoke('restore_from_clipboard')) }, t('restoreClip')))));
  }
  if (b.signerChange) {
    const c = b.signerChange;
    out.push(banner(true, t('changeTitle'),
      h('p', {}, t('tradeoff')),
      h('p', {}, h('strong', {}, t('oldPeg') + ': '), h('span', { class: 'mono' }, c.trustedPeg)),
      h('p', {}, h('strong', {}, t('newPeg') + ': '), h('span', { class: 'mono' }, c.reportedPeg)),
      h('p', { class: 'small' }, t('keysChanged', { a: c.added.length, r: c.removed.length })),
      h('p', {}, h('strong', {}, t('checkIt'))),
      h('ul', {}, c.links.map((l, i) => h('li', {}, link('check', String(i), t(`sources.${l.source}`)))))));
  }
  if (b.error) {
    out.push(banner(true, t('bannerUntrusted'), h('p', {}, b.error.message), h('p', {}, t('bannerUntrustedText'))));
  }
  if (view.connectionError) out.push(banner(false, t('bannerOffline'), h('p', {}, view.connectionError)));
  if (b.paused) out.push(banner(false, t('bannerPaused')));
  if (b.solvent === false) out.push(banner(true, t('bannerInsolvent')));
  if (view.hasWallet && view.receiveBlocked) {
    out.push(banner(false, t('bannerBackup'), h('p', {}, t('bannerBackupText')),
      h('div', { class: 'row' }, h('button', { class: 'primary', type: 'button', onclick: () => act(() => invoke('backup')) }, t('backupNow')))));
  }
  $('banners').replaceChildren(...out);
}

// --- the main area -------------------------------------------------------------

function renderWelcome() {
  return h('section', { class: 'card welcome' },
    h('span', { class: 'mark-wrap' },
      h('img', { src: 'madblocks.svg', alt: '', class: 'mark' }),
      h('img', { src: 'bitcoin.svg', alt: '', class: 'coin-badge' })),
    h('h1', {}, t('welcomeTitle')),
    h('p', { class: 'muted' }, t('welcomeText')),
    h('div', { class: 'row' },
      h('button', { class: 'primary', type: 'button', onclick: () => act(() => invoke('create_wallet')) }, t('create')),
      h('button', { type: 'button', onclick: () => act(() => invoke('import_from_clipboard')) }, t('importClip'))),
    h('p', { class: 'small muted' }, t('helloTwice')),
    h('p', { class: 'small muted' }, t('importHint')));
}

function renderWalletSkeleton() {
  return [
    h('section', { class: 'card', id: 'address-card' }),
    h('div', { class: 'grid2', id: 'balances' }),
    h('section', { class: 'card', id: 'actions' }, renderActions()),
    h('section', { class: 'card', id: 'inflight' }),
    h('section', { class: 'card', id: 'history' }),
    h('section', { class: 'card', id: 'bridge' }),
  ];
}

function field(id, labelKey, attrs = {}) {
  return [h('label', { for: id }, t(labelKey)), h('input', { id, autocomplete: 'off', spellcheck: 'false', ...attrs })];
}

/** An amount, with a button that fills in the most the action can move. */
function amountField(id, placeholder, action) {
  return [
    h('label', { for: id }, t('amount')),
    h('div', { class: 'amount-row' },
      h('input', { id, autocomplete: 'off', spellcheck: 'false', inputmode: 'decimal', placeholder }),
      h('button', { type: 'button', title: t('maxHint'), onclick: () => fillMax(id, action) }, t('max'))),
  ];
}

async function fillMax(id, action) {
  const value = (field) => ($(field) ? $(field).value : '');
  const chain = action === 'send' ? value('send-chain') : '';
  const to = action === 'send' ? value('send-to') : action === 'withdraw' ? value('withdraw-to') : '';
  const max = await act(() => invoke('max_amount', { action, chain, to }));
  if (max) $(id).value = max;
}

function renderActions() {
  const tabs = h('div', { class: 'tabs' }, ['send', 'deposit', 'withdraw'].map((k) =>
    h('button', { type: 'button', class: tab === k ? 'on' : '', onclick: () => { tab = k; rebuildActions(); } }, t(k))));
  let body;
  if (tab === 'send') {
    body = h('div', {},
      h('label', { for: 'send-chain' }, t('network')),
      h('select', { id: 'send-chain' }, h('option', { value: 'btcvm' }, 'BTCVM'), h('option', { value: 'bitcoin' }, 'Bitcoin')),
      ...field('send-to', 'to', { placeholder: 'bc1q…' }),
      ...amountField('send-amount', '0.0001', 'send'),
      h('p', { class: 'small muted' }, t('sendHint')),
      h('div', { class: 'row' }, h('button', { class: 'primary', type: 'button', onclick: () => prepare('send') }, t('review'))));
  } else if (tab === 'deposit') {
    body = h('div', {},
      ...amountField('deposit-amount', '0.0005', 'deposit'),
      h('p', { class: 'small muted', id: 'deposit-hint' }),
      h('p', { class: 'small', id: 'deposit-paused' }),
      h('div', { class: 'row' }, h('button', { class: 'primary', type: 'button', id: 'deposit-go', onclick: () => prepare('deposit') }, t('review'))));
  } else {
    body = h('div', {},
      ...field('withdraw-to', 'btcAddress', { placeholder: 'bc1q…' }),
      h('div', { class: 'row' }, h('button', { class: 'link', type: 'button', onclick: () => { if (view.address) $('withdraw-to').value = view.address; } }, t('useMine'))),
      ...amountField('withdraw-amount', '0.0002', 'withdraw'),
      h('p', { class: 'small muted', id: 'withdraw-hint' }),
      h('p', { class: 'small', id: 'withdraw-paused' }),
      h('div', { class: 'row' }, h('button', { class: 'primary', type: 'button', id: 'withdraw-go', onclick: () => prepare('withdraw') }, t('review'))));
  }
  return [tabs, body];
}

function rebuildActions() {
  const card = $('actions');
  if (card) {
    card.replaceChildren(...renderActions());
    updateActions();
  }
}

function updateActions() {
  const b = view.bridge;
  const paused = !!b.signerChange;
  const limits = [b.minDeposit && t('limits', { min: b.minDeposit }), b.maxDeposit && t('maxCap', { max: b.maxDeposit }), b.vmFee && t('vmFeeNote', { fee: b.vmFee })].filter(Boolean).join(' · ');
  if ($('deposit-hint')) {
    $('deposit-hint').textContent = `${t('depositHint')} ${limits}`;
    $('deposit-paused').textContent = paused ? t('paused') : '';
    $('deposit-go').disabled = paused || !b.trusted;
  }
  if ($('withdraw-hint')) {
    $('withdraw-hint').textContent = `${t('withdrawHint', { fee: b.payoutFee || '?' })} ${b.minPegOut ? t('limits', { min: b.minPegOut }) : ''}`;
    $('withdraw-paused').textContent = paused ? t('paused') : '';
    $('withdraw-go').disabled = paused || !b.trusted;
  }
}

function balanceCard(cls, title, bal, note) {
  return h('section', { class: `card balance ${cls}` },
    h('h2', {}, h('img', { src: cls === 'vm' ? 'btcvm.svg' : 'bitcoin.svg', alt: '', class: 'coin' }), title),
    bal
      ? h('div', {}, h('div', { class: 'amount' }, bal.confirmed, ' ', h('small', {}, 'BTC')),
        bal.pending && !/^-?0(\.0*)?$/.test(bal.pending) ? h('div', { class: 'small muted' }, `${t('pending')}: ${bal.pending} BTC`) : null)
      : h('div', { class: 'muted' }, note ? t(note) : t('loading')));
}

function updateWallet() {
  const card = $('address-card');
  card.replaceChildren(h('h2', {}, t('yourAddress')),
    view.address
      ? h('div', {},
        h('div', { class: 'mono amount', id: 'address' }, view.address),
        h('p', { class: 'small muted' }, t('sameAddress')),
        h('div', { class: 'row' },
          h('button', { type: 'button', onclick: () => invoke('copy_address').then(() => toast(t('copied'))).catch(showError) }, t('copy')),
          link('address-bitcoin', view.address, t('onBitcoin')),
          link('address-btcvm', view.address, t('onBtcvm'))))
      : h('p', { class: 'muted' }, t('blockedReceive')));

  $('balances').replaceChildren(
    balanceCard('btc', t('bitcoin'), view.bitcoin, view.bitcoinNote),
    balanceCard('vm', t('btcvm'), view.btcvm, null));

  updateActions();

  const items = [];
  for (const o of view.inFlight) {
    items.push(h('li', {}, h('span', {}, h('span', { class: o.chain === 'btcvm' ? 'tag vm' : 'tag' }, o.chain === 'btcvm' ? 'BTCVM' : 'Bitcoin'), ' ',
      t(`kinds.${o.kind}`), ' · ', `${(o.amount / 1e8).toFixed(8).replace(/\.?0+$/, '')} BTC → `, h('span', { class: 'mono' }, short(o.to))),
    link(o.chain === 'btcvm' ? 'tx-btcvm' : 'tx-bitcoin', o.txid, t('view'))));
  }
  for (const w of view.withdrawals.filter((w) => (w.paymentConfirmations || 0) === 0 && w.status !== 'unknown')) {
    const status = t(`withdrawalStatus.${w.status}`, { pays: w.pays || '' });
    items.push(h('li', {}, h('span', {}, t('withdrawalLabel', { to: short(w.to) }), ' · ', status),
      w.paymentTxid ? link('tx-bitcoin', w.paymentTxid, t('view')) : link('tx-btcvm', w.txid, t('view'))));
  }
  for (const d of view.deposits.filter((d) => d.status !== 'credited' || !d.creditTxid)) {
    items.push(h('li', {}, h('span', {}, t('depositLabel'), ' · ', `${d.amount} BTC · `,
      t(`depositStatus.${d.status}`, { c: d.confirmations, r: d.required, amt: d.credited || d.amount }), d.reason ? ` (${d.reason})` : ''),
    link('tx-bitcoin', d.txid, t('view'))));
  }
  $('inflight').replaceChildren(h('h2', {}, t('inFlight')),
    items.length ? h('ul', { class: 'list' }, items) : h('p', { class: 'muted' }, t('nothingInFlight')));

  $('history').replaceChildren(h('h2', {}, t('history')),
    view.history.length
      ? h('ul', { class: 'list' }, view.history.map((r) => h('li', {},
        h('span', {}, h('span', { class: r.chain === 'btcvm' ? 'tag vm' : 'tag' }, r.chain === 'btcvm' ? 'BTCVM' : 'Bitcoin'), ' ',
          h('span', { class: r.net.startsWith('-') ? '' : 'plus' }, `${r.net.startsWith('-') ? '' : '+'}${r.net} BTC`), ' · ',
          h('span', { class: 'muted small' }, r.confirmations > 0 ? `${when(r.time)} · ${t('confs', { n: r.confirmations })}` : t('unconfirmed'))),
        link(r.chain === 'btcvm' ? 'tx-btcvm' : 'tx-bitcoin', r.txid, t('view')))))
      : h('p', { class: 'muted' }, t('noHistory')));

  const b = view.bridge;
  $('bridge').replaceChildren(h('h2', {}, t('bridge')),
    b.pegAddress ? h('p', { class: 'small' }, `${t('pegPinned')}: `, h('span', { class: 'mono' }, b.pegAddress)) : null,
    b.locked ? h('p', { class: 'small' }, t('audit', { locked: b.locked, circ: b.circulating }), ' — ', b.solvent ? t('solvent') : t('notSolvent')) : null,
    b.feeRate ? h('p', { class: 'small muted' }, t('feeRate', { rate: b.feeRate })) : null,
    h('p', { class: 'small muted' }, view.server));
}

function apply(next) {
  view = next;
  lang = view.language === 'es' ? 'es' : 'en';
  renderChrome();
  renderBanners();
  const nextMode = view.hasWallet ? 'wallet' : view.vaultProblem ? 'problem' : 'welcome';
  if (nextMode !== mode || apply.lang !== lang) {
    mode = nextMode;
    apply.lang = lang;
    $('main').replaceChildren(...(mode === 'wallet' ? renderWalletSkeleton() : mode === 'welcome' ? [renderWelcome()] : []));
  }
  if (mode === 'wallet') updateWallet();
}

// --- review and signing ------------------------------------------------------------

async function prepare(kind) {
  const value = (id) => ($(id) ? $(id).value.trim() : '');
  const review = await act(() => {
    toast(t('working'));
    if (kind === 'send') return invoke('prepare_send', { chain: value('send-chain'), to: value('send-to'), amount: value('send-amount') });
    if (kind === 'deposit') return invoke('prepare_deposit', { amount: value('deposit-amount') });
    return invoke('prepare_withdrawal', { to: value('withdraw-to'), amount: value('withdraw-amount') });
  });
  if (review) showReview(review);
}

function showReview(r) {
  $('toast').hidden = true;
  const chainName = r.chain === 'btcvm' ? 'BTCVM' : 'Bitcoin';
  const rows = r.outputs.map((o) => h('tr', {},
    h('td', {}, h('div', {}, t(`role.${o.role}`)),
      o.address ? h('div', { class: 'mono small' }, o.address) : null,
      o.withdrawalTo ? h('div', { class: 'mono small' }, o.withdrawalTo) : null),
    h('td', { class: 'num' }, o.role === 'tag' ? '' : `${o.value} BTC`)));
  const status = h('p', { class: 'small muted' });
  const confirmButton = h('button', { class: 'primary', type: 'button' }, t('confirm'));
  const cancelButton = h('button', { type: 'button' }, t('cancel'));
  confirmButton.addEventListener('click', async () => {
    confirmButton.disabled = true;
    cancelButton.disabled = true;
    status.textContent = t('waitingHello');
    try {
      const sent = await invoke('confirm', { id: r.id });
      $('modal').close();
      toast(t('sent', { txid: short(sent.txid) }));
      for (const id of ['send-amount', 'deposit-amount', 'withdraw-amount']) if ($(id)) $(id).value = '';
    } catch (e) {
      status.textContent = '';
      showError(e);
      confirmButton.disabled = false;
      cancelButton.disabled = false;
    }
  });
  cancelButton.addEventListener('click', () => { invoke('cancel', { id: r.id }); $('modal').close(); });
  openModal(
    h('h2', {}, t(`reviewTitle.${r.kind}`, { chain: chainName })),
    h('table', {}, h('tbody', {}, rows,
      h('tr', {}, h('td', {}, t('fee')), h('td', { class: 'num' }, `${r.fee} BTC`)),
      h('tr', { class: 'total' }, h('td', {}, t('total')), h('td', { class: 'num' }, `${r.total} BTC`)),
      r.credited ? h('tr', {}, h('td', {}, t('credited'), r.confirmations ? ` (${t('afterConf', { n: r.confirmations })})` : ''), h('td', { class: 'num' }, `${r.credited} BTC`)) : null,
      r.payoutFee ? h('tr', {}, h('td', {}, t('payoutFee')), h('td', { class: 'num' }, `${r.payoutFee} BTC`)) : null)),
    status,
    h('div', { class: 'row spread' }, cancelButton, confirmButton));
}

function openModal(...content) {
  $('modal-body').replaceChildren(...content);
  if (!$('modal').open) $('modal').showModal();
}

// --- settings and about ------------------------------------------------------------

function openSettings() {
  const server = h('input', { id: 'server', value: view.server, spellcheck: 'false' });
  const language = h('select', { id: 'language' }, h('option', { value: 'es' }, 'Español'), h('option', { value: 'en' }, 'English'));
  language.value = lang;
  language.addEventListener('change', () => act(() => invoke('set_language', { language: language.value })).then(openSettings));
  openModal(
    h('h2', {}, t('settings')),
    h('label', { for: 'language' }, t('language')), language,
    h('label', { for: 'server' }, t('server')), server,
    h('div', { class: 'row' },
      h('button', { type: 'button', onclick: () => act(() => invoke('set_server', { url: server.value })) }, t('save')),
      h('button', { type: 'button', onclick: () => act(() => invoke('set_server', { url: '' })).then(openSettings) }, t('reset'))),
    view.hasWallet ? h('div', { class: 'row' },
      h('button', { type: 'button', onclick: () => act(() => invoke('backup')) }, t('backupKey')),
      h('button', { class: 'danger', type: 'button', onclick: () => act(() => invoke('remove_wallet')) }, t('removeWallet'))) : null,
    h('h3', {}, t('security')),
    h('p', { class: 'small' }, t('securityText')),
    h('p', { class: 'small' }, t('tradeoff')),
    h('div', { class: 'row spread' }, h('span'), h('button', { type: 'button', onclick: () => $('modal').close() }, t('close'))));
}

async function openAbout() {
  if (!aboutInfo) aboutInfo = await invoke('about_info');
  openModal(
    h('div', { class: 'row' }, h('img', { src: 'madblocks.svg', alt: '', class: 'mark' }), h('h2', {}, t('aboutTitle'))),
    h('p', {}, t('aboutText')),
    h('div', { class: 'about-links' },
      h('button', { type: 'button', onclick: () => invoke('open_link', { kind: 'vote', arg: '' }) }, t('vote')),
      h('button', { type: 'button', onclick: () => invoke('open_link', { kind: 'metal', arg: '' }) }, t('delegate', { node: aboutInfo.metalNodeId })),
      h('button', { type: 'button', onclick: () => invoke('open_link', { kind: 'website', arg: '' }) }, t('website')),
      h('button', { type: 'button', onclick: () => invoke('open_link', { kind: 'x', arg: '' }) }, t('xLink'))),
    h('p', { class: 'foot' }, t('disclaimer')),
    h('p', { class: 'foot' }, t('license', { v: aboutInfo.version })),
    h('div', { class: 'row spread' }, h('span'), h('button', { type: 'button', onclick: () => $('modal').close() }, t('close'))));
}

// --- start -------------------------------------------------------------------------

$('lang').addEventListener('click', () => act(() => invoke('set_language', { language: lang === 'es' ? 'en' : 'es' })));
$('open-settings').addEventListener('click', () => view && openSettings());
$('open-about').addEventListener('click', () => openAbout().catch(showError));

listen('view', (event) => apply(event.payload));
invoke('hello', { language: navigator.language || 'en' }).then(apply).catch(showError);
invoke('refresh').then(apply).catch(showError);
