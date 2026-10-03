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
    backupKey: 'Ver la clave de la cartera en uso (Windows Hello)', removeWallet: 'Quitar la cartera en uso de este PC',
    security: 'Seguridad',
    securityText: 'La clave se descifra solo para firmar, tras Windows Hello, y nunca llega a esta ventana. Las direcciones de depósito y la reserva del puente se calculan aquí a partir de los firmantes fijados en la wallet; el servidor no puede redirigir un pago. Cada moneda se comprueba contra la transacción que la creó y las comisiones tienen tope.',
    aboutTitle: 'Acerca de',
    aboutText: 'madblocks BTCVM Wallet la ha creado madblocks, block producer de XPR Network y validador de Metal Blockchain. Es gratuita; si te resulta útil, apoya a madblocks:',
    vote: 'Votar a madblocks en XPR Network', delegate: 'Delegar en el validador de Metal ({node})', website: 'madblocks.tech', xLink: 'Seguir a @madblocksbp en X',
    disclaimer: 'Wallet independiente: no la ha hecho ni la avala Metallicus. Usa la API pública del puente de BTCVM. Software sin auditar y BTCVM está en alfa: usa cantidades pequeñas.',
    license: 'Licencia MIT. Versión {v}.', close: 'Cerrar',
    wallets: 'Carteras', book: 'Libreta', mainWallet: 'Principal', walletN: 'Cartera {id}',
    manageWallets: 'Tus carteras',
    walletsText: 'Cada cartera es una clave distinta, con su propia dirección (la misma en Bitcoin y en BTCVM), su copia de seguridad y su clave de Windows Hello. Para mover fondos entre ellas, envía a la dirección de la otra: aparecen en «Elegir de la libreta…».',
    active: 'en uso', use: 'Usar', rename: 'Renombrar', newWalletName: 'Nombre de la nueva cartera (opcional)', namePlaceholder: 'Ej.: Ahorro',
    newWallet: 'Crear cartera nueva', importWallet: 'Importar clave del portapapeles', needsBackup: 'sin copia de seguridad',
    bookTitle: 'Libreta de direcciones',
    bookText: 'Nombres para las direcciones a las que pagas. Guarda cada una para la red en la que la usas: la misma dirección existe en Bitcoin y en BTCVM, pero un exchange que solo vigila Bitcoin no verá lo que le envíes en BTCVM.',
    noContacts: 'Todavía no hay direcciones guardadas.', contactName: 'Nombre', contactAddress: 'Dirección', addContact: 'Guardar en la libreta', delete: 'Borrar', newName: 'Nuevo nombre',
    pick: 'Elegir de la libreta…', myWallets: 'Mis carteras', contactsFor: 'Libreta ({chain})', otherNetwork: 'guardadas para la otra red',
    feeLabel: 'Comisión de Bitcoin', feeFast: 'Rápida (~10 min)', feeHalfHour: 'Normal (~30 min)', feeHour: 'Lenta (~1 h)', feeEconomy: 'Económica (horas)', feeBridge: 'Recomendada (la del puente)', feeCustom: 'Personalizada (sat/vB)',
    feeSlow: 'Con esta comisión puede tardar horas o días en confirmarse. Tus fondos no se pierden mientras tanto.',
    feeBelowMin: 'Está por debajo del mínimo que aceptan hoy los nodos: probablemente la red la rechace y no se envíe.',
    from: 'Desde', feeRateUsed: '{rate} sat/vB',
    toOwn: 'Destino: tu cartera «{name}».', toBook: 'Destino: «{name}», de tu libreta.',
    toBookOther: '«{name}» está guardado en tu libreta para la otra red. Si es de un exchange que solo vigila esa red, no verá este pago.',
    toNew: 'Dirección nueva: no está en tu libreta. Compruébala carácter a carácter con la que te dieron.',
    lookalike: '¡Atención! Esta dirección se parece a «{name}» (de tu libreta o tus carteras) pero NO es la misma. Puede ser un intento de suplantación (address poisoning): no copies direcciones del historial.',
    feeInvalid: 'Escribe la comisión en sat/vB: un número entero del 1 al {max}.',
    feeVmFixed: 'En BTCVM la comisión es fija y mínima: 1 sat por cada 1000 vB (como poco, 1 sat). Solo se elige en Bitcoin.',
    feeHelp: 'Se paga por tamaño (sat por vbyte), no por cantidad. Una comisión baja no pone en riesgo tus fondos: solo tarda más en confirmarse. Si queda por debajo del mínimo de la red, se rechaza y no sale nada.',
    backupShort: 'Copia de seguridad', removeShort: 'Quitar', sure: '¿Seguro? Pulsa otra vez',
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
    backupKey: 'Show the key of the wallet in use (Windows Hello)', removeWallet: 'Remove the wallet in use from this PC',
    security: 'Security',
    securityText: 'The key is decrypted only to sign, after Windows Hello, and never reaches this window. Deposit addresses and the bridge\'s reserve are computed here from the signers pinned in the wallet; the server can\'t redirect a payment. Every coin is checked against the transaction that created it, and fees are capped.',
    aboutTitle: 'About',
    aboutText: 'madblocks BTCVM Wallet is made by madblocks, XPR Network block producer and Metal Blockchain validator. It\'s free; if it\'s useful to you, support madblocks:',
    vote: 'Vote for madblocks on XPR Network', delegate: 'Delegate to the Metal validator ({node})', website: 'madblocks.tech', xLink: 'Follow @madblocksbp on X',
    disclaimer: 'An independent wallet: not made or endorsed by Metallicus. It uses BTCVM\'s public bridge API. Unaudited software, and BTCVM is in alpha: keep amounts small.',
    license: 'MIT license. Version {v}.', close: 'Close',
    wallets: 'Wallets', book: 'Address book', mainWallet: 'Main', walletN: 'Wallet {id}',
    manageWallets: 'Your wallets',
    walletsText: 'Each wallet is a different key, with its own address (the same on Bitcoin and BTCVM), its own backup and its own Windows Hello key. To move funds between them, send to the other\'s address: they appear under "Pick from the address book…".',
    active: 'in use', use: 'Use', rename: 'Rename', newWalletName: 'Name of the new wallet (optional)', namePlaceholder: 'e.g. Savings',
    newWallet: 'Create a new wallet', importWallet: 'Import a key from the clipboard', needsBackup: 'not backed up',
    bookTitle: 'Address book',
    bookText: 'Names for the addresses you pay. Save each for the network you use it on: the same address exists on Bitcoin and BTCVM, but an exchange that only watches Bitcoin won\'t see what you send it on BTCVM.',
    noContacts: 'No saved addresses yet.', contactName: 'Name', contactAddress: 'Address', addContact: 'Save to the address book', delete: 'Delete', newName: 'New name',
    pick: 'Pick from the address book…', myWallets: 'My wallets', contactsFor: 'Address book ({chain})', otherNetwork: 'saved for the other network',
    feeLabel: 'Bitcoin fee', feeFast: 'Fast (~10 min)', feeHalfHour: 'Normal (~30 min)', feeHour: 'Slow (~1 h)', feeEconomy: 'Economy (hours)', feeBridge: 'Recommended (the bridge\'s)', feeCustom: 'Custom (sat/vB)',
    feeSlow: 'At this fee it may take hours or days to confirm. Your funds aren\'t lost meanwhile.',
    feeBelowMin: 'It\'s under the minimum nodes accept today: the network will probably refuse it and it won\'t be sent.',
    from: 'From', feeRateUsed: '{rate} sat/vB',
    toOwn: 'To your wallet "{name}".', toBook: 'To "{name}", from your address book.',
    toBookOther: '"{name}" is saved in your address book for the other network. If it\'s an exchange that only watches that network, it won\'t see this payment.',
    toNew: 'A new address: it isn\'t in your address book. Check it character by character against the one you were given.',
    lookalike: 'Careful! This address looks like "{name}" (from your address book or your wallets) but is NOT the same. It may be an impersonation attempt (address poisoning): don\'t copy addresses from your history.',
    feeInvalid: 'Enter the fee in sat/vB: a whole number from 1 to {max}.',
    feeVmFixed: 'On BTCVM the fee is fixed and tiny: 1 sat per 1000 vB (at least 1 sat). It\'s only chosen on Bitcoin.',
    feeHelp: 'You pay by size (sat per vbyte), not by amount. A low fee doesn\'t put your funds at risk: it only takes longer to confirm. Under the network\'s minimum, it\'s refused and nothing is sent.',
    backupShort: 'Back up', removeShort: 'Remove', sure: 'Sure? Click again',
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

// The toast is a popover, so it shows above an open dialog too.
function toast(message, bad = false) {
  const el = $('toast');
  el.textContent = message;
  el.className = bad ? 'bad' : '';
  if (el.showPopover) {
    if (el.matches(':popover-open')) el.hidePopover();
    el.showPopover();
  } else el.hidden = false;
  clearTimeout(toast.timer);
  toast.timer = setTimeout(hideToast, bad ? 9000 : 5000);
}

function hideToast() {
  const el = $('toast');
  if (!el.hidePopover) el.hidden = true;
  else if (el.matches(':popover-open')) el.hidePopover();
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
  const has = !!(view && view.hasWallet);
  for (const id of ['wallet-select', 'open-wallets', 'open-book']) $(id).hidden = !has;
  if (has) {
    const select = $('wallet-select');
    const key = JSON.stringify([lang, view.wallets.map((w) => [w.id, w.name])]);
    if (select.dataset.key !== key) {
      select.dataset.key = key;
      select.replaceChildren(...view.wallets.map((w) => h('option', { value: w.id }, walletName(w))));
    }
    select.value = view.walletId || '';
  }
}

function banner(bad, title, ...body) {
  return h('div', { class: bad ? 'banner bad' : 'banner' }, h('h3', {}, title), ...body);
}

/** A warning of one paragraph, styled as a banner. */
function notice(bad, text) {
  return h('div', { class: bad ? 'banner bad' : 'banner' }, h('p', {}, text));
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
      h('button', { class: 'primary', type: 'button', onclick: () => act(() => invoke('create_wallet', { name: '' })) }, t('create')),
      h('button', { type: 'button', onclick: () => act(() => invoke('import_from_clipboard', { name: '' })) }, t('importClip'))),
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
  const max = await act(() => invoke('max_amount', { action, chain, to, feeRate: action === 'withdraw' ? null : chosenFee(action) }));
  if (max) {
    $(id).value = max;
    // Remembered, so a change of fee or network sizes it again.
    $(id).dataset.max = max;
  }
}

/** Sizes again an amount Max filled in, unless it was edited since. */
function refreshMax(action) {
  const field = $(`${action}-amount`);
  if (field && field.value && field.dataset.max === field.value) fillMax(field.id, action);
}

function renderActions() {
  const tabs = h('div', { class: 'tabs' }, ['send', 'deposit', 'withdraw'].map((k) =>
    h('button', { type: 'button', class: tab === k ? 'on' : '', onclick: () => { tab = k; rebuildActions(); } }, t(k))));
  let body;
  if (tab === 'send') {
    body = h('div', {},
      h('label', { for: 'send-chain' }, t('network')),
      h('select', { id: 'send-chain', onchange: () => { showSendFee(); refillPickers(); refreshMax('send'); } },
        h('option', { value: 'btcvm' }, 'BTCVM'), h('option', { value: 'bitcoin' }, 'Bitcoin')),
      h('label', { for: 'send-to' }, t('to')),
      picker('send-to', () => ($('send-chain') ? $('send-chain').value : 'btcvm')),
      h('input', { id: 'send-to', autocomplete: 'off', spellcheck: 'false', placeholder: 'bc1q…' }),
      ...amountField('send-amount', '0.0001', 'send'),
      feeBlock('send'),
      h('p', { class: 'small muted', id: 'send-vm-fee' }, t('feeVmFixed')),
      h('p', { class: 'small muted' }, t('sendHint')),
      h('div', { class: 'row' }, h('button', { class: 'primary', type: 'button', onclick: () => prepare('send') }, t('review'))));
  } else if (tab === 'deposit') {
    body = h('div', {},
      ...amountField('deposit-amount', '0.0005', 'deposit'),
      feeBlock('deposit'),
      h('p', { class: 'small muted', id: 'deposit-hint' }),
      h('p', { class: 'small', id: 'deposit-paused' }),
      h('div', { class: 'row' }, h('button', { class: 'primary', type: 'button', id: 'deposit-go', onclick: () => prepare('deposit') }, t('review'))));
  } else {
    body = h('div', {},
      h('label', { for: 'withdraw-to' }, t('btcAddress')),
      picker('withdraw-to', () => 'bitcoin'),
      h('input', { id: 'withdraw-to', autocomplete: 'off', spellcheck: 'false', placeholder: 'bc1q…' }),
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
  showSendFee();
  for (const prefix of ['send', 'deposit']) { fillFees(prefix); updateFeeNote(prefix); }
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
  const shown = (view.wallets || []).find((w) => w.active);
  card.replaceChildren(h('h2', {}, `${t('yourAddress')} · ${walletName(shown)}`),
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
  refillPickers();

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
    if (kind === 'send') return invoke('prepare_send', { chain: value('send-chain'), to: value('send-to'), amount: value('send-amount'), feeRate: chosenFee('send') });
    if (kind === 'deposit') return invoke('prepare_deposit', { amount: value('deposit-amount'), feeRate: chosenFee('deposit') });
    return invoke('prepare_withdrawal', { to: value('withdraw-to'), amount: value('withdraw-amount') });
  });
  if (review) showReview(review);
}

function showReview(r) {
  hideToast();
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
  const notes = [];
  if (r.lookalike) notes.push(notice(true, t('lookalike', { name: r.lookalike })));
  if (r.toKnown === 'own') notes.push(h('p', { class: 'small' }, t('toOwn', { name: r.toName })));
  else if (r.toKnown === 'book') notes.push(h('p', { class: 'small' }, t('toBook', { name: r.toName })));
  else if (r.toKnown === 'bookOtherChain') notes.push(notice(false, t('toBookOther', { name: r.toName })));
  else if (r.toKnown === 'new') notes.push(h('p', { class: 'small muted' }, t('toNew')));
  openModal(
    h('h2', {}, t(`reviewTitle.${r.kind}`, { chain: chainName })),
    h('p', { class: 'small muted' }, `${t('from')}: ${r.walletName}`),
    ...notes,
    h('table', {}, h('tbody', {}, rows,
      h('tr', {}, h('td', {}, t('fee'), r.feeRate ? ` (${t('feeRateUsed', { rate: r.feeRate })})` : ''), h('td', { class: 'num' }, `${r.fee} BTC`)),
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

// --- wallets, the address book and fees --------------------------------------------

/** A wallet's name: its own, or the default (the Rust side gives the same). */
function walletName(w) {
  if (!w) return '';
  if (w.name) return w.name;
  return w.id === 'main' ? t('mainWallet') : t('walletN', { id: (w.id || '').slice(0, 4) });
}

const chainLabel = (c) => (c === 'btcvm' ? 'BTCVM' : 'Bitcoin');

/** A list that fills in a destination: the other wallets, then the address
 *  book, with the entries saved for the network paid on first. */
function picker(inputId, chainOf) {
  const select = h('select', { class: 'picker', 'aria-label': t('pick') });
  select.refill = () => {
    if (!view) return;
    const chain = chainOf();
    const own = (view.wallets || []).filter((w) => !w.active && w.address);
    const book = view.addressBook || [];
    const key = JSON.stringify([lang, chain, own.map((w) => [w.address, walletName(w)]), book]);
    if (select.dataset.key === key) return;
    select.dataset.key = key;
    const same = book.filter((c) => c.chain === chain);
    const other = book.filter((c) => c.chain !== chain);
    const option = (address, label) => h('option', { value: address }, `${label} — ${short(address)}`);
    select.replaceChildren(...[
      h('option', { value: '' }, t('pick')),
      own.length && h('optgroup', { label: t('myWallets') }, own.map((w) => option(w.address, walletName(w)))),
      same.length && h('optgroup', { label: t('contactsFor', { chain: chainLabel(chain) }) }, same.map((c) => option(c.address, c.name))),
      other.length && h('optgroup', { label: `${t('book')} (${t('otherNetwork')})` }, other.map((c) => option(c.address, `${c.name} · ${chainLabel(c.chain)}`))),
    ].filter(Boolean));
    select.hidden = !(own.length || book.length);
  };
  select.addEventListener('change', () => {
    if (select.value && $(inputId)) $(inputId).value = select.value;
    select.value = '';
  });
  select.refill();
  return select;
}

function refillPickers() {
  for (const s of document.querySelectorAll('select.picker')) if (s.refill) s.refill();
}

let feeOptions = null;

const FEE_CHOICES = [['bridge', 'feeBridge'], ['fastest', 'feeFast'], ['halfHour', 'feeHalfHour'], ['hour', 'feeHour'], ['economy', 'feeEconomy']];

/** The sat/vB of a named choice: mempool.space's, or the bridge's estimate. */
function feeRateOf(key) {
  if (feeOptions && feeOptions[key]) return feeOptions[key];
  return key === 'bridge' && view && view.bridge.feeRate ? view.bridge.feeRate : null;
}

/** The Bitcoin fee to pay: the bridge's estimate, mempool.space's speeds, or
 *  a rate typed in. The core refuses one out of bounds, and the review shows
 *  the rate used. */
function feeBlock(prefix) {
  const select = h('select', { id: `${prefix}-fee` });
  const custom = h('input', { id: `${prefix}-fee-custom`, inputmode: 'numeric', placeholder: 'sat/vB', autocomplete: 'off', 'aria-label': t('feeCustom') });
  select.addEventListener('change', () => {
    custom.hidden = select.value !== 'custom';
    updateFeeNote(prefix);
    refreshMax(prefix);
  });
  custom.addEventListener('input', () => updateFeeNote(prefix));
  custom.addEventListener('change', () => refreshMax(prefix));
  fillFees(prefix, select, custom);
  return h('div', { id: `${prefix}-fee-block` },
    h('label', { for: `${prefix}-fee` }, t('feeLabel')),
    h('div', { class: 'amount-row' }, select, custom),
    h('p', { class: 'small', id: `${prefix}-fee-note` }),
    h('p', { class: 'small muted' }, t('feeHelp')));
}

function fillFees(prefix, select = $(`${prefix}-fee`), custom = $(`${prefix}-fee-custom`)) {
  if (!select) return;
  const choices = FEE_CHOICES.filter(([key]) => feeRateOf(key)).map(([key, label]) => [key, `${t(label)}: ${feeRateOf(key)} sat/vB`]);
  const key = JSON.stringify(choices);
  if (select.dataset.key !== key) {
    select.dataset.key = key;
    const keep = select.value;
    select.replaceChildren(...choices.map(([value, text]) => h('option', { value }, text)), h('option', { value: 'custom' }, t('feeCustom')));
    if ([...select.options].some((o) => o.value === keep)) select.value = keep;
  }
  custom.hidden = select.value !== 'custom';
}

/** The fee rate chosen, sat/vB, or null where none is chosen (BTCVM). A rate
 *  typed in that isn't valid throws, for `act` to show. */
function chosenFee(prefix) {
  const select = $(`${prefix}-fee`);
  if (!select || select.closest('[hidden]')) return null;
  if (select.value !== 'custom') return feeRateOf(select.value);
  const max = (feeOptions && feeOptions.max) || 1000;
  const raw = $(`${prefix}-fee-custom`).value.trim();
  const rate = /^\d{1,7}$/.test(raw) ? Number(raw) : 0;
  if (rate < 1 || rate > max) throw new Error(t('feeInvalid', { max }));
  return rate;
}

function updateFeeNote(prefix) {
  const note = $(`${prefix}-fee-note`);
  if (!note) return;
  let rate = null;
  try { rate = chosenFee(prefix); } catch { rate = null; }
  const o = feeOptions || {};
  const belowMin = rate != null && o.minimum && rate < o.minimum;
  const slow = rate != null && o.hour && rate < o.hour;
  note.textContent = belowMin ? t('feeBelowMin') : slow ? t('feeSlow') : '';
  note.className = belowMin ? 'small bad-text' : 'small warn-text';
}

async function loadFees() {
  try { feeOptions = await invoke('fee_options'); } catch { feeOptions = null; }
  for (const prefix of ['send', 'deposit']) { fillFees(prefix); updateFeeNote(prefix); }
}

/** The fee is chosen only on Bitcoin; BTCVM's is fixed. */
function showSendFee() {
  const bitcoin = !!$('send-chain') && $('send-chain').value === 'bitcoin';
  if ($('send-fee-block')) $('send-fee-block').hidden = !bitcoin;
  if ($('send-vm-fee')) $('send-vm-fee').hidden = bitcoin;
}

/** A button that asks for a second click before it acts. */
function twoStep(label, onConfirm) {
  const b = h('button', { class: 'danger', type: 'button' }, label);
  b.addEventListener('click', () => {
    if (b.dataset.armed) return onConfirm();
    b.dataset.armed = '1';
    b.textContent = t('sure');
    setTimeout(() => { delete b.dataset.armed; b.textContent = label; }, 4000);
  });
  return b;
}

function openWallets() {
  const after = (v) => { if (v) openWallets(); };
  const rows = (view.wallets || []).map((w) => h('li', {},
    h('div', { class: 'grow' },
      h('div', {}, h('strong', {}, walletName(w)), w.active ? h('span', { class: 'tag vm' }, t('active')) : null),
      h('div', { class: 'mono small muted' }, w.address || t('needsBackup')),
      h('div', { class: 'small muted' }, `Bitcoin ${w.bitcoin ? w.bitcoin.confirmed : '…'} BTC · BTCVM ${w.btcvm ? w.btcvm.confirmed : '…'} BTC`)),
    h('div', { class: 'row' },
      w.active
        ? [h('button', { type: 'button', onclick: () => act(() => invoke('backup')).then(after) }, t('backupShort')),
          h('button', { class: 'danger', type: 'button', onclick: () => act(() => invoke('remove_wallet')).then((v) => { if (v) (v.hasWallet ? openWallets() : $('modal').close()); }) }, t('removeShort'))]
        : h('button', { type: 'button', onclick: () => act(() => invoke('select_wallet', { id: w.id })).then((v) => { if (v) $('modal').close(); }) }, t('use')),
      h('button', { type: 'button', onclick: () => renameWalletDialog(w) }, t('rename')))));
  const name = h('input', { id: 'new-wallet-name', placeholder: t('namePlaceholder'), maxlength: '60', autocomplete: 'off' });
  openModal(
    h('h2', {}, t('manageWallets')),
    h('p', { class: 'small muted' }, t('walletsText')),
    h('ul', { class: 'list' }, rows),
    h('label', { for: 'new-wallet-name' }, t('newWalletName')), name,
    h('div', { class: 'row' },
      h('button', { class: 'primary', type: 'button', onclick: () => act(() => invoke('create_wallet', { name: name.value })).then(after) }, t('newWallet')),
      h('button', { type: 'button', onclick: () => act(() => invoke('import_from_clipboard', { name: name.value })).then(after) }, t('importWallet'))),
    h('p', { class: 'small muted' }, t('helloTwice')),
    h('p', { class: 'small muted' }, t('importHint')),
    h('div', { class: 'row spread' }, h('span'), h('button', { type: 'button', onclick: () => $('modal').close() }, t('close'))));
}

function renameWalletDialog(w) {
  const input = h('input', { id: 'rename-wallet', value: w.name || '', placeholder: walletName(w), maxlength: '60', autocomplete: 'off' });
  openModal(
    h('h2', {}, `${t('rename')}: ${walletName(w)}`),
    h('label', { for: 'rename-wallet' }, t('newName')), input,
    h('div', { class: 'row spread' },
      h('button', { type: 'button', onclick: () => openWallets() }, t('cancel')),
      h('button', { class: 'primary', type: 'button', onclick: () => act(() => invoke('rename_wallet', { id: w.id, name: input.value })).then((v) => { if (v) openWallets(); }) }, t('save'))));
  input.focus();
}

function openBook() {
  const book = view.addressBook || [];
  const rows = book.map((c) => h('li', {},
    h('div', { class: 'grow' },
      h('div', {}, h('strong', {}, c.name), h('span', { class: c.chain === 'btcvm' ? 'tag vm' : 'tag' }, chainLabel(c.chain))),
      h('div', { class: 'mono small' }, c.address)),
    h('div', { class: 'row' },
      h('button', { type: 'button', onclick: () => renameContactDialog(c) }, t('rename')),
      twoStep(t('delete'), () => act(() => invoke('remove_contact', { address: c.address, chain: c.chain })).then((v) => { if (v) openBook(); })))));
  const name = h('input', { id: 'c-name', maxlength: '60', autocomplete: 'off' });
  const address = h('input', { id: 'c-address', spellcheck: 'false', autocomplete: 'off', placeholder: 'bc1q…' });
  const chain = h('select', { id: 'c-chain' }, h('option', { value: 'bitcoin' }, 'Bitcoin'), h('option', { value: 'btcvm' }, 'BTCVM'));
  openModal(
    h('h2', {}, t('bookTitle')),
    h('p', { class: 'small muted' }, t('bookText')),
    book.length ? h('ul', { class: 'list' }, rows) : h('p', { class: 'muted' }, t('noContacts')),
    h('h3', {}, t('addContact')),
    h('label', { for: 'c-name' }, t('contactName')), name,
    h('label', { for: 'c-address' }, t('contactAddress')), address,
    h('label', { for: 'c-chain' }, t('network')), chain,
    h('div', { class: 'row spread' },
      h('button', { type: 'button', onclick: () => $('modal').close() }, t('close')),
      h('button', { class: 'primary', type: 'button', onclick: () => act(() => invoke('add_contact', { name: name.value, address: address.value, chain: chain.value })).then((v) => { if (v) openBook(); }) }, t('addContact'))));
}

function renameContactDialog(c) {
  const input = h('input', { id: 'rename-contact', value: c.name, maxlength: '60', autocomplete: 'off' });
  openModal(
    h('h2', {}, `${t('rename')}: ${c.name}`),
    h('p', { class: 'mono small' }, `${chainLabel(c.chain)} · ${c.address}`),
    h('label', { for: 'rename-contact' }, t('newName')), input,
    h('div', { class: 'row spread' },
      h('button', { type: 'button', onclick: () => openBook() }, t('cancel')),
      h('button', { class: 'primary', type: 'button', onclick: () => act(() => invoke('rename_contact', { address: c.address, chain: c.chain, name: input.value })).then((v) => { if (v) openBook(); }) }, t('save'))));
  input.focus();
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
$('wallet-select').addEventListener('change', (e) => act(() => invoke('select_wallet', { id: e.target.value })).then(renderChrome));
$('open-wallets').addEventListener('click', () => view && openWallets());
$('open-book').addEventListener('click', () => view && openBook());
if (!$('toast').showPopover) $('toast').hidden = true;
loadFees();
setInterval(loadFees, 10 * 60 * 1000);

listen('view', (event) => apply(event.payload));
invoke('hello', { language: navigator.language || 'en' }).then(apply).catch(showError);
invoke('refresh').then(apply).catch(showError);
