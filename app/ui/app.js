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
    welcomeText: 'Una clave, la misma dirección en Bitcoin y en BTCVM, y Windows Hello para cada pago. Tu cartera tendrá una frase de 12 palabras para recuperarla; se crea en este PC y se guarda cifrada con una clave de Windows Hello que vive en el TPM.',
    create: 'Crear una wallet nueva', importClip: 'Importar mis palabras o mi clave desde el portapapeles',
    importHint: 'Para importar, copia tu frase de 12 o 24 palabras, o tu clave WIF (empieza por K o L), y pulsa el botón: se lee aquí y el portapapeles se borra.',
    helloTwice: 'Al crear o importar tu primera cartera, Windows Hello te pedirá el PIN dos veces: una para crear su clave en el TPM y otra para cifrar la tuya con ella. Las carteras siguientes lo piden una vez, y cada pago, una vez.',
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
    reviewTitle: { send: 'Enviar en {chain}', deposit: 'Pasar a BTCVM', withdraw: 'Retirar a Bitcoin', bump: 'Acelerar un pago en Bitcoin' },
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
    restoreHint: 'Copia tu frase de recuperación o tu clave WIF y pulsa el botón. Tiene que ser la de esta cartera.',
    bannerUntrusted: 'El puente no ha pasado las comprobaciones de la wallet', bannerUntrustedText: 'No se firmará nada con este puente. Puede ser un error del servidor o un ataque.',
    bannerOffline: 'No se puede conectar con el puente', bannerPaused: 'Los operadores han pausado el puente.',
    bannerInsolvent: 'La auditoría del puente indica que el peg no está totalmente cubierto. No muevas fondos entre cadenas.',
    bannerBackup: 'Haz la copia de seguridad de tu clave', bannerBackupText: 'Hasta entonces no se muestra tu dirección: este PC tiene la única copia de la clave.', backupNow: 'Hacer copia (Windows Hello)',
    changeTitle: 'Han cambiado los firmantes del puente',
    tradeoff: 'La contrapartida: si cambian los firmantes del puente, los depósitos y las retiradas se pausan. Una rotación planificada por los operadores de BTCVM se ve igual que un servidor secuestrado. Por eso la wallet busca la transacción con la que los firmantes antiguos trasladan los fondos al nuevo conjunto y comprueba sus firmas: si la encuentra, se actualiza sola y todo vuelve a funcionar. Mientras no la encuentre, no mueve fondos entre cadenas, te avisa y te dice dónde comprobarlo. Los envíos siguen funcionando. Es intencionado.',
    oldPeg: 'Peg fijado en la wallet', newPeg: 'Peg que informa ahora el puente', keysChanged: '{a} claves nuevas, {r} retiradas',
    checkIt: 'Dónde comprobarlo',
    sources: { oldPegOnBitcoin: 'El peg antiguo en un explorador de Bitcoin: tras una rotación real, sus BTC se han movido al nuevo (solo los firmantes antiguos podían hacerlo)', newPegOnBitcoin: 'El peg nuevo en un explorador de Bitcoin', rotationProcedure: 'Cómo rotan los firmantes los operadores de BTCVM', btcvmDocs: 'Documentación de BTCVM (metalbtc.com)', btcvmExplorer: 'Explorador de BTCVM (metalbtc.com)', walletMaker: 'madblocks publica cada conjunto de firmantes verificado con las actualizaciones' },
    server: 'Servidor del puente', save: 'Guardar', reset: 'Por defecto', language: 'Idioma',
    backupKey: 'Ver la copia de la cartera en uso (Windows Hello)', removeWallet: 'Quitar la cartera en uso de este PC',
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
    newWallet: 'Crear cartera nueva', importWallet: 'Importar palabras o clave del portapapeles', needsBackup: 'sin copia de seguridad',
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
    incomingDeposit: 'pendiente: +{amount} BTC de tu depósito, cuando el puente lo acredite',
    internalTitle: 'Algo ha fallado dentro de la wallet',
    internalText: 'La wallet sigue funcionando y no se ha firmado ni enviado nada por ello. El detalle está en el registro; si se repite, envíaselo a madblocks (no contiene claves).',
    openLogs: 'Abrir el registro',
    showQr: 'QR', qrTitle: 'Tu dirección en QR',
    qrText: 'La misma dirección en Bitcoin y en BTCVM: quien te pague elige la red. Comprueba en su pantalla que la dirección que ha leído es esta:',
    exportCsv: 'Exportar CSV', exported: 'Historial guardado',
    currency: 'Mostrar también los valores en', currencyNone: 'No mostrar (no se consulta el precio)',
    received: 'Recibido: +{amount} BTC en {chain}', receivedPending: 'Llegando: +{amount} BTC en {chain} (sin confirmar)',
    tpmNo: 'sin TPM certificado',
    softwareKeyTitle: 'Windows no puede certificar que la clave de esta cartera esté en un chip TPM',
    softwareKeyText: 'Tu clave sigue cifrada y cada pago pide Windows Hello, pero la llave que la abre podría estar guardada por software en lugar de en el chip de seguridad. Pasa en equipos sin TPM o con uno antiguo. Un malware con permisos de administrador lo tendría más fácil: para cantidades grandes, usa un equipo con TPM 2.0.',
    understood: 'Entendido',
    checkBalances: 'Comprobar también mi saldo de Bitcoin con mempool.space (mempool.space conocerá tu dirección)',
    disagreeTitle: 'El puente y mempool.space no coinciden en tu saldo de Bitcoin',
    disagreeText: 'El puente dice {bridge} BTC confirmados y mempool.space {other} BTC. Puede ser un bloque que acaba de llegar; si persiste, el puente podría estar mostrando datos falsos. Tus fondos no corren riesgo por ello: cada pago se comprueba antes de firmarlo.',
    checkNow: 'Buscar la prueba ahora',
    rotationTitle: 'Firmantes del puente actualizados',
    rotationText: 'Los firmantes anteriores firmaron el traslado de los fondos del peg {from} al nuevo {to}, y la wallet ha comprobado sus firmas. Los depósitos y las retiradas vuelven a funcionar con el nuevo conjunto.',
    viewMove: 'Ver la transacción',
    bump: 'Acelerar', bumpTitle: 'Acelerar un pago',
    bumpText: 'Se envía otra vez con una comisión más alta: las mismas monedas y el mismo pago, y la diferencia sale de tu cambio. Los nodos lo aceptan porque tus pagos lo permiten (RBF). Cuando se confirme uno de los dos, el otro deja de valer: nunca se paga dos veces.',
    previousFee: 'Comisión anterior',
    checkTitle: 'Comprueba tu copia',
    checkWordsText: 'Para asegurarte de que la has apuntado bien, escribe estas palabras de tu frase. Son solo unas pocas: no bastan para reconstruir tu clave.',
    checkCharsText: 'Para asegurarte de que la has apuntado bien, escribe estos caracteres de tu clave. Son solo unos pocos: no bastan para reconstruirla.',
    checkWord: 'Palabra n.º {n}', checkChars: 'Caracteres del {a} al {b}',
    notNow: 'Ahora no', showAgain: 'Ver la copia otra vez', check: 'Comprobar', checkOk: 'Copia comprobada',
    tpmStatus: 'Clave de Windows Hello de la cartera en uso', tpmCertified: 'en un chip TPM (certificado por Windows)', tpmNotCertified: 'Windows no certifica que esté en un TPM', tpmUnknown: 'comprobando…',
  },
  en: {
    byline: 'by madblocks · XPR Network BP and Metal validator',
    about: 'About', settings: 'Settings', connected: 'Connected', offline: 'Offline',
    welcomeTitle: 'Your Bitcoin and BTCVM wallet',
    welcomeText: 'One key, the same address on Bitcoin and BTCVM, and Windows Hello for every payment. Your wallet gets a 12-word phrase to restore it; it is made on this PC and kept encrypted to a Windows Hello key that lives in the TPM.',
    create: 'Create a new wallet', importClip: 'Import my words or my key from the clipboard',
    importHint: 'To import, copy your 12- or 24-word phrase, or your WIF key (starting with K or L), and press the button: it is read here and the clipboard is cleared.',
    helloTwice: 'When you create or import your first wallet, Windows Hello asks for your PIN twice: once to make its key in the TPM and once to encrypt yours with it. Later wallets ask once, and each payment once.',
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
    reviewTitle: { send: 'Send on {chain}', deposit: 'Move to BTCVM', withdraw: 'Withdraw to Bitcoin', bump: 'Speed up a payment on Bitcoin' },
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
    restoreHint: 'Copy your recovery phrase or your WIF key and press the button. It must be this wallet\'s.',
    bannerUntrusted: 'The bridge failed the wallet\'s checks', bannerUntrustedText: 'Nothing will be signed with this bridge. It may be a server fault or an attack.',
    bannerOffline: 'Can\'t reach the bridge', bannerPaused: 'The operators have paused the bridge.',
    bannerInsolvent: 'The bridge\'s audit shows the peg isn\'t fully backed. Don\'t move coins between the chains.',
    bannerBackup: 'Back up your key', bannerBackupText: 'Until then your address isn\'t shown: this PC holds the only copy of the key.', backupNow: 'Back up now (Windows Hello)',
    changeTitle: 'The bridge\'s signers have changed',
    tradeoff: 'The trade-off: if the bridge\'s signers change, deposits and withdrawals pause. A planned rotation by BTCVM\'s operators looks the same as a hijacked server. So the wallet looks for the transaction in which the old signers move the funds to the new set, and checks their signatures: if it finds it, it updates itself and everything works again. Until then, it moves no coins between the chains, warns you and shows where to check. Sends keep working. This is deliberate.',
    oldPeg: 'Peg pinned in the wallet', newPeg: 'Peg the bridge reports now', keysChanged: '{a} keys added, {r} removed',
    checkIt: 'Where to check it',
    sources: { oldPegOnBitcoin: 'The old peg on a Bitcoin explorer: after a real rotation its BTC have moved to the new one (only the old signers could do that)', newPegOnBitcoin: 'The new peg on a Bitcoin explorer', rotationProcedure: 'How BTCVM\'s operators rotate the signers', btcvmDocs: 'BTCVM\'s documentation (metalbtc.com)', btcvmExplorer: 'BTCVM\'s explorer (metalbtc.com)', walletMaker: 'madblocks publish each verified signer set with the wallet\'s updates' },
    server: 'Bridge server', save: 'Save', reset: 'Default', language: 'Language',
    backupKey: 'Show the backup of the wallet in use (Windows Hello)', removeWallet: 'Remove the wallet in use from this PC',
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
    newWallet: 'Create a new wallet', importWallet: 'Import words or a key from the clipboard', needsBackup: 'not backed up',
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
    incomingDeposit: 'pending: +{amount} BTC from your deposit, once the bridge credits it',
    internalTitle: 'Something went wrong inside the wallet',
    internalText: 'The wallet keeps working, and nothing was signed or sent because of it. The details are in the log; if it happens again, send it to madblocks (it holds no keys).',
    openLogs: 'Open the log',
    showQr: 'QR', qrTitle: 'Your address as a QR code',
    qrText: 'The same address on Bitcoin and on BTCVM: whoever pays you chooses the network. Check on their screen that the address they read is this one:',
    exportCsv: 'Export CSV', exported: 'History saved',
    currency: 'Also show values in', currencyNone: 'Don\'t show (the price isn\'t fetched)',
    received: 'Received: +{amount} BTC on {chain}', receivedPending: 'Coming in: +{amount} BTC on {chain} (unconfirmed)',
    tpmNo: 'no certified TPM',
    softwareKeyTitle: 'Windows can\'t certify that this wallet\'s key is in a TPM chip',
    softwareKeyText: 'Your key is still encrypted and every payment asks for Windows Hello, but the key that opens it may be kept in software rather than in the security chip. That happens on PCs without a TPM or with an old one. Malware with administrator rights would have an easier time: for large amounts, use a PC with TPM 2.0.',
    understood: 'Got it',
    checkBalances: 'Also check my Bitcoin balance with mempool.space (mempool.space will learn your address)',
    disagreeTitle: 'The bridge and mempool.space disagree on your Bitcoin balance',
    disagreeText: 'The bridge says {bridge} BTC confirmed and mempool.space {other} BTC. It may be a block that just arrived; if it lasts, the bridge may be showing false data. Your funds aren\'t at risk from it: every payment is checked before it is signed.',
    checkNow: 'Look for the proof now',
    rotationTitle: 'The bridge\'s signers were updated',
    rotationText: 'The previous signers signed the move of the funds from the peg {from} to the new {to}, and the wallet checked their signatures. Deposits and withdrawals work again with the new set.',
    viewMove: 'View the transaction',
    bump: 'Speed up', bumpTitle: 'Speed up a payment',
    bumpText: 'It is sent again with a higher fee: the same coins and the same payment, the difference coming out of your change. Nodes accept it because your payments allow it (RBF). Once one of the two confirms, the other is void: it is never paid twice.',
    previousFee: 'Previous fee',
    checkTitle: 'Check your backup',
    checkWordsText: 'To make sure you wrote it down right, type these words of your phrase. They are only a few: not enough to rebuild your key.',
    checkCharsText: 'To make sure you wrote it down right, type these characters of your key. They are only a few: not enough to rebuild it.',
    checkWord: 'Word #{n}', checkChars: 'Characters {a} to {b}',
    notNow: 'Not now', showAgain: 'Show the backup again', check: 'Check', checkOk: 'Backup checked',
    tpmStatus: 'Windows Hello key of the wallet in use', tpmCertified: 'in a TPM chip (certified by Windows)', tpmNotCertified: 'Windows doesn\'t certify it\'s in a TPM', tpmUnknown: 'checking…',
  },
};

// The Rust side's messages in Spanish. {} marks a part that varies; in the
// Spanish, {0} is the first such part and {0*} translates it too. A message
// not listed (an answer from the bridge's server, say) shows as it came.
const escapeRe = (text) => text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
const ERRORS_ES = [
  // First: it wraps another message, and patterns below end with "{}".
  ['{}. It may or may not have been sent: it stays in the list of payments in flight, and its coins aren\'t offered again until it settles. Its id is {}.', '{0*}. Puede que se haya enviado o no: sigue en la lista de pagos en curso y sus monedas no se ofrecen de nuevo hasta que se aclare. Su id es {1}.'],
  // The bridge and the network.
  ['the bridge isn\'t connected yet', 'el puente aún no está conectado'],
  ['can\'t reach the bridge: {}', 'no se puede conectar con el puente: {0}'],
  ['the bridge\'s answer didn\'t parse: {}', 'la respuesta del puente no se pudo leer: {0}'],
  ['the bridge answered {}', 'el puente respondió {0}'],
  ['no event stream', 'no hay flujo de eventos'],
  ['the bridge\'s address must be https://, like https://metalbtc.com', 'la dirección del puente debe empezar por https://, como https://metalbtc.com'],
  ['the bridge is for {} and {} (chain {}), not the networks this wallet is for', 'el puente es de {0} y {1} (cadena {2}), no de las redes de esta wallet'],
  ['the bridge\'s {} address doesn\'t follow from its signers', 'la dirección {0} del puente no sale de sus firmantes'],
  ['the bridge\'s fee rate, {} sat/vB, looks wrong', 'la comisión del puente, {0} sat/vB, parece un error'],
  ['the bridge reports address formats that aren\'t the network\'s', 'el puente informa de formatos de dirección que no son los de la red'],
  ['the bridge\'s deposit limits contradict each other', 'los límites de depósito del puente se contradicen'],
  ['the bridge\'s signer set is invalid: {}', 'el conjunto de firmantes del puente no es válido: {0*}'],
  ['the bridge\'s {} isn\'t an amount', 'el dato «{0}» del puente no es una cantidad'],
  ['the bridge gave a deposit address that doesn\'t follow from the peg\'s signers, so nothing was sent', 'el puente dio una dirección de depósito que no sale de los firmantes del peg, así que no se ha enviado nada'],
  ['the bridge reported transaction {}, but this wallet signed {}', 'el puente informó de la transacción {0}, pero esta wallet firmó {1}'],
  ['the bridge\'s signers have changed, so moving coins between Bitcoin and BTCVM is paused until the wallet is updated with the new set; sends still work', 'los firmantes del puente han cambiado, así que mover fondos entre Bitcoin y BTCVM queda en pausa hasta que la wallet se actualice con el nuevo conjunto; los envíos siguen funcionando'],
  ['signer key is not hex', 'la clave de un firmante no está en hexadecimal'],
  ['a signer key appears twice', 'una clave de firmante aparece dos veces'],
  ['invalid signer set', 'conjunto de firmantes no válido'],
  ['signer keys must be compressed public keys', 'las claves de los firmantes deben ser claves públicas comprimidas'],
  // Payments.
  ['that is the bridge\'s own address; use Move to BTCVM, which pays your personal deposit address', 'esa es la dirección del propio puente; usa «Pasar a BTCVM», que paga a tu dirección de depósito personal'],
  ['that is the bridge\'s reserve; use Withdraw to Bitcoin, which tags the payment with your Bitcoin address', 'esa es la reserva del puente; usa «Retirar a Bitcoin», que etiqueta el pago con tu dirección de Bitcoin'],
  ['that is the bridge\'s own address; it can\'t be paid directly', 'esa es la dirección del propio puente; no se le puede pagar directamente'],
  ['the smallest deposit is {} BTC; a smaller one is not credited', 'el depósito mínimo es {0} BTC; uno menor no se acredita'],
  ['a deposit can be at most {} BTC for now; a larger one is held for a refund', 'por ahora un depósito puede ser como mucho de {0} BTC; uno mayor queda retenido para devolverlo'],
  ['the smallest withdrawal is {} BTC; a smaller one is not paid', 'la retirada mínima es {0} BTC; una menor no se paga'],
  ['a withdrawal can\'t pay the bridge\'s own address', 'una retirada no puede pagar a la dirección del propio puente'],
  ['the wallet spends only its own native SegWit coins', 'la wallet solo gasta sus propias monedas SegWit nativas'],
  ['the smallest payment is {} BTC', 'el pago mínimo es {0} BTC'],
  ['more than 21 million BTC', 'más de 21 millones de BTC'],
  ['a fee rate of {} sat/vB looks wrong; refusing', 'una comisión de {0} sat/vB parece un error; no se firma'],
  ['coin values overflow', 'los valores de las monedas se desbordan'],
  ['not enough confirmed BTC on {}: have {}, need {} including the fee', 'no hay suficientes BTC confirmados en {0}: tienes {1} y hacen falta {2} con la comisión'],
  ['not enough confirmed BTC on {} to cover the fee', 'no hay suficientes BTC confirmados en {0} para cubrir la comisión'],
  ['the fee would be {} BTC; refusing to sign', 'la comisión sería de {0} BTC; no se firma'],
  ['after the fee there are {} BTC, under the minimum of {} BTC', 'tras la comisión quedan {0} BTC, por debajo del mínimo de {1} BTC'],
  ['no transaction {}', 'no se encuentra la transacción {0}'],
  ['transaction {} is not hex', 'la transacción {0} no está en hexadecimal'],
  ['the server sent the wrong transaction for {}', 'el servidor envió una transacción equivocada para {0}'],
  ['transaction {} has no output {}', 'la transacción {0} no tiene la salida {1}'],
  ['output {} is not yours', 'la salida {0} no es tuya'],
  ['bad txid {}', 'id de transacción incorrecto: {0}'],
  ['transaction {}: {}', 'transacción {0}: {1*}'],
  ['this key does not own the plan\'s coins', 'esta clave no es la dueña de las monedas del pago'],
  ['the signed transaction differs from the one reviewed; nothing was sent', 'la transacción firmada no coincide con la revisada; no se ha enviado nada'],
  ['the signed transaction didn\'t match what you reviewed, so it wasn\'t sent', 'la transacción firmada no coincidía con lo que revisaste, así que no se ha enviado'],
  ['the signed transaction isn\'t hex', 'la transacción firmada no está en hexadecimal'],
  ['trailing bytes after the transaction', 'sobran bytes tras la transacción'],
  ['truncated transaction', 'transacción incompleta'],
  ['transaction too large', 'transacción demasiado grande'],
  ['that payment is no longer waiting; prepare it again', 'ese pago ya no está pendiente; prepáralo de nuevo'],
  ['the active wallet changed; prepare the payment again', 'ha cambiado la cartera en uso; prepara el pago de nuevo'],
  ['this wallet\'s balance hasn\'t loaded yet', 'el saldo de esta cartera aún no se ha cargado'],
  ['your {} balance hasn\'t loaded yet', 'tu saldo de {0} aún no se ha cargado'],
  ['unknown action', 'acción desconocida'],
  ['unknown network', 'red desconocida'],
  ['enter an amount like 0.0025', 'escribe una cantidad como 0.0025'],
  // Addresses and keys.
  ['malformed destination', 'destino mal formado'],
  ['unsupported SegWit address', 'dirección SegWit no admitida'],
  ['address is for a different network', 'la dirección es de otra red'],
  ['not an address', 'no es una dirección'],
  ['not a valid address', 'no es una dirección válida'],
  ['push too large', 'dato demasiado grande'],
  ['checksum mismatch: check for a typo', 'la suma de control no cuadra: revisa si hay una errata'],
  ['invalid padding', 'relleno no válido'],
  ['mixed-case address', 'dirección con mayúsculas y minúsculas mezcladas'],
  ['too short', 'demasiado corta'],
  ['not base58', 'no es base58'],
  ['invalid character in address', 'carácter no válido en la dirección'],
  ['a private key is 32 bytes', 'una clave privada tiene 32 bytes'],
  ['not a valid private key', 'no es una clave privada válida'],
  ['this is an uncompressed-key WIF; export a compressed one', 'es una WIF de clave sin comprimir; exporta una comprimida'],
  ['not a private key', 'no es una clave privada'],
  ['not a transaction id', 'no es un id de transacción'],
  // The address book and wallets.
  ['give the address a name', 'ponle un nombre a la dirección'],
  ['a name can be at most {} characters', 'un nombre puede tener como mucho {0} caracteres'],
  ['already saved as "{}"', 'ya está guardada como «{0}»'],
  ['the address book is full: it holds {} addresses; delete one first', 'la libreta está llena: caben {0} direcciones; borra una antes'],
  ['that address is no longer in the book', 'esa dirección ya no está en la libreta'],
  ['that key is already in this app, as wallet "{}"', 'esa clave ya está en la app, como la cartera «{0}»'],
  ['there is no such wallet', 'esa cartera no existe'],
  ['there is no wallet on this PC', 'no hay ninguna cartera en este PC'],
  ['this wallet\'s vault can\'t be read', 'no se puede leer la bóveda de esta cartera'],
  ['the key in the vault isn\'t this wallet\'s address; nothing was signed', 'la clave de la bóveda no es la de la dirección de esta cartera; no se ha firmado nada'],
  ['back up your key before receiving', 'haz la copia de seguridad de tu clave antes de recibir'],
  ['copy your recovery phrase or your key (WIF) first, then press the button', 'copia primero tu frase de recuperación o tu clave (WIF) y luego pulsa el botón'],
  ['there is no backup to check', 'no hay ninguna copia que comprobar'],
  ['that transaction doesn\'t move the peg to the new signers (no BVMM tag naming them)', 'esa transacción no traslada el peg a los nuevos firmantes (no lleva la marca BVMM que los nombra)'],
  ['no input of that transaction carries the old signers\' signatures', 'ninguna entrada de esa transacción lleva las firmas de los firmantes antiguos'],
  ['not a signer set\'s multisig script', 'no es el script multifirma de un conjunto de firmantes'],
  ['no such input', 'esa entrada no existe'],
  ['that payment isn\'t waiting any more', 'ese pago ya no está pendiente'],
  ['this payment can\'t be replaced: it doesn\'t signal BIP 125', 'este pago no se puede sustituir: no lo permite (BIP 125)'],
  ['that payment pays out more than it spends', 'ese pago gasta más de lo que tiene'],
  ['this payment has no change to pay a higher fee from', 'este pago no tiene cambio del que sacar una comisión más alta'],
  ['the change of {} BTC can\'t pay {} BTC more in fees', 'el cambio de {0} BTC no alcanza para pagar {1} BTC más de comisión'],
  ['that doesn\'t match the backup you were shown: look at it again, or show it again', 'no coincide con la copia que se te mostró: vuelve a mirarla, o muéstrala otra vez'],
  ['a recovery phrase has 12, 15, 18, 21 or 24 words', 'una frase de recuperación tiene 12, 15, 18, 21 o 24 palabras'],
  ['word {} isn\'t one of the BIP 39 words', 'la palabra {0} no es una de las palabras BIP 39'],
  ['the recovery phrase\'s checksum doesn\'t match: check each word and their order', 'la suma de control de la frase no cuadra: revisa cada palabra y su orden'],
  ['a recovery phrase\'s entropy is 16 to 32 bytes, in steps of 4', 'la entropía de una frase es de 16 a 32 bytes, de 4 en 4'],
  ['this phrase derives an unusable key; use another', 'esta frase da una clave inutilizable; usa otra'],
  ['the sealed secret has the wrong length', 'el secreto sellado no tiene la longitud correcta'],
  ['no such link', 'ese enlace no existe'],
  ['unknown currency', 'moneda desconocida'],
  ['the QR code: {}', 'el código QR: {0}'],
  ['there is no log yet', 'todavía no hay registro'],
  ['something went wrong inside the wallet; nothing was signed or sent. The details are in the log (Settings)', 'algo ha fallado dentro de la wallet; no se ha firmado ni enviado nada. El detalle está en el registro (Ajustes)'],
  // The vault and Windows Hello.
  ['the wallet\'s vault is damaged: {}', 'la bóveda de la cartera está dañada: {0*}'],
  ['the Windows Hello key that unlocks this wallet is gone; restore the wallet from its backup', 'la clave de Windows Hello que abre esta cartera ya no existe; restáurala desde su copia de seguridad'],
  ['Windows Hello was canceled', 'se ha cancelado Windows Hello'],
  ['this PC already has a wallet; remove it first', 'este PC ya tiene una cartera; quítala antes'],
  ['Windows Hello isn\'t set up on this PC: add a PIN in Settings > Accounts > Sign-in options', 'Windows Hello no está configurado en este PC: añade un PIN en Configuración > Cuentas > Opciones de inicio de sesión'],
  ['Windows Hello\'s signature doesn\'t verify as RSA PKCS#1 v1.5; refusing to use it', 'la firma de Windows Hello no se verifica como RSA PKCS#1 v1.5; no se usa'],
  ['the PC\'s security device is locked after too many attempts; try again later', 'el dispositivo de seguridad del PC está bloqueado tras demasiados intentos; inténtalo más tarde'],
  ['a Windows Hello key with this name already exists', 'ya existe una clave de Windows Hello con este nombre'],
  ['Windows Hello failed (status {})', 'Windows Hello ha fallado (estado {0})'],
  ['that key is for {}, not this wallet ({}); remove the wallet first to use a different key', 'esa clave es de {0}, no de esta cartera ({1}); quita la cartera antes para usar otra clave'],
  ['the vault didn\'t read back the same; nothing was stored', 'la bóveda no se leyó igual al comprobarla; no se ha guardado nada'],
  ['it is unlocked by {}, not {}', 'se abre con {0}, no con {1}'],
  ['the key inside isn\'t the wallet\'s address', 'la clave que contiene no es la de la dirección de la cartera'],
  ['not a vault file: {}', 'no es un archivo de bóveda: {0}'],
  ['the vault\'s file: {}', 'el archivo de la bóveda: {0}'],
  ['the nonce is not 12 bytes', 'el nonce no tiene 12 bytes'],
  ['the vault doesn\'t open with this Windows Hello key', 'la bóveda no se abre con esta clave de Windows Hello'],
  ['the sealed key is not 32 bytes', 'la clave sellada no tiene 32 bytes'],
  ['not a vault this version understands ({})', 'no es una bóveda que entienda esta versión ({0})'],
  ['the {} is not hex', 'el campo {0} no está en hexadecimal'],
  ['LOCALAPPDATA is not set', 'LOCALAPPDATA no está definida'],
].map(([en, es]) => [new RegExp(`^${en.split('{}').map(escapeRe).join('(.+?)')}$`, 's'), es]);

let lang = 'en';
let view = null;
let aboutInfo = null;
let mode = null;
let tab = 'send';
let busy = false;
/** Called when the dialog closes, unless replaced first. */
let modalOnClose = null;
/** 'check' while the dialog asks for the backup back. */
let modalKind = null;
let checkShown = null;

function t(key, vars) {
  let s = key.split('.').reduce((o, k) => (o == null ? o : o[k]), STRINGS[lang]);
  if (s == null) s = key.split('.').reduce((o, k) => (o == null ? o : o[k]), STRINGS.en) ?? key;
  if (vars) for (const [k, v] of Object.entries(vars)) s = s.replaceAll(`{${k}}`, String(v));
  return s;
}

/** A message from the Rust side, in Spanish when the window is. */
function tr(message) {
  if (lang !== 'es' || typeof message !== 'string') return message;
  for (const [pattern, es] of ERRORS_ES) {
    const m = message.match(pattern);
    if (m) return es.replace(/\{(\d)(\*?)\}/g, (_, i, deep) => (deep ? tr(m[Number(i) + 1]) : m[Number(i) + 1]));
  }
  return message;
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

/** A BTC amount's value in the chosen currency, or null. */
function fiat(btc) {
  if (!view || !view.fiat || btc == null) return null;
  const value = Number(btc) * view.fiat.price;
  if (!Number.isFinite(value)) return null;
  return new Intl.NumberFormat(lang === 'es' ? 'es-ES' : 'en-US', { style: 'currency', currency: view.fiat.currency }).format(value);
}
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
  const message = tr((e && e.message) || String(e));
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
    out.push(banner(true, t('bannerVault'), h('p', {}, tr(view.vaultProblem)), h('p', {}, t('restoreHint')),
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
      h('ul', {}, c.links.map((l, i) => h('li', {}, link('check', String(i), t(`sources.${l.source}`))))),
      h('div', { class: 'row' }, h('button', { type: 'button', onclick: () => act(() => invoke('check_rotation')) }, t('checkNow')))));
  }
  if (b.rotation) {
    const r = b.rotation;
    out.push(h('div', { class: 'banner ok' }, h('h3', {}, t('rotationTitle')),
      h('p', {}, t('rotationText', { from: short(r.fromPeg), to: short(r.toPeg) })),
      h('div', { class: 'row' },
        link(r.chain === 'btcvm' ? 'tx-btcvm' : 'tx-bitcoin', r.txid, t('viewMove')),
        h('button', { type: 'button', onclick: () => act(() => invoke('rotation_seen')) }, t('understood')))));
  }
  if (b.error) {
    out.push(banner(true, t('bannerUntrusted'), h('p', {}, tr(b.error.message)), h('p', {}, t('bannerUntrustedText'))));
  }
  if (view.internalError) {
    out.push(banner(true, t('internalTitle'), h('p', {}, t('internalText')), h('p', { class: 'small mono' }, view.internalError),
      h('div', { class: 'row' }, h('button', { type: 'button', onclick: () => invoke('open_logs').catch(showError) }, t('openLogs')))));
  }
  if (view.connectionError) out.push(banner(false, t('bannerOffline'), h('p', {}, tr(view.connectionError))));
  if (view.bitcoinDisagrees && view.bitcoin) {
    out.push(banner(false, t('disagreeTitle'), h('p', {}, t('disagreeText', { bridge: view.bitcoin.confirmed, other: view.bitcoinDisagrees }))));
  }
  if (b.paused) out.push(banner(false, t('bannerPaused')));
  if (b.solvent === false) out.push(banner(true, t('bannerInsolvent')));
  if (view.hasWallet && view.softwareKeyWarning) {
    out.push(banner(false, t('softwareKeyTitle'), h('p', {}, t('softwareKeyText')),
      h('div', { class: 'row' }, h('button', { type: 'button', onclick: () => act(() => invoke('software_key_seen')) }, t('understood')))));
  }
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

/** A balance, with what is pending: unconfirmed payments, and on BTCVM the
 *  deposits the bridge hasn't credited yet. */
function balanceCard(cls, title, bal, note, incoming) {
  return h('section', { class: `card balance ${cls}` },
    h('h2', {}, h('img', { src: cls === 'vm' ? 'btcvm.svg' : 'bitcoin.svg', alt: '', class: 'coin' }), title),
    bal
      ? h('div', {}, h('div', { class: 'amount' }, bal.confirmed, ' ', h('small', {}, 'BTC')),
        fiat(bal.confirmed) ? h('div', { class: 'small muted' }, `≈ ${fiat(bal.confirmed)}`) : null,
        bal.pending && !/^-?0(\.0*)?$/.test(bal.pending) ? h('div', { class: 'small muted' }, `${t('pending')}: ${bal.pending} BTC`) : null,
        incoming ? h('div', { class: 'small muted' }, t('incomingDeposit', { amount: incoming })) : null)
      : h('div', { class: 'muted' }, note ? tr(t(note)) : t('loading')));
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
          h('button', { type: 'button', onclick: openQr }, t('showQr')),
          link('address-bitcoin', view.address, t('onBitcoin')),
          link('address-btcvm', view.address, t('onBtcvm'))))
      : h('p', { class: 'muted' }, t('blockedReceive')));

  $('balances').replaceChildren(
    balanceCard('btc', t('bitcoin'), view.bitcoin, view.bitcoinNote),
    balanceCard('vm', t('btcvm'), view.btcvm, null, view.btcvmIncoming));

  updateActions();
  refillPickers();

  const items = [];
  for (const o of view.inFlight) {
    items.push(h('li', {}, h('span', {}, h('span', { class: o.chain === 'btcvm' ? 'tag vm' : 'tag' }, o.chain === 'btcvm' ? 'BTCVM' : 'Bitcoin'), ' ',
      t(`kinds.${o.kind}`), ' · ', `${(o.amount / 1e8).toFixed(8).replace(/\.?0+$/, '')} BTC → `, h('span', { class: 'mono' }, short(o.to))),
    h('span', { class: 'row' },
      o.chain === 'bitcoin' ? h('button', { class: 'link', type: 'button', onclick: () => openBump(o) }, t('bump')) : null,
      link(o.chain === 'btcvm' ? 'tx-btcvm' : 'tx-bitcoin', o.txid, t('view')))));
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

  $('history').replaceChildren(
    h('div', { class: 'row spread' }, h('h2', {}, t('history')), view.history.length ? h('button', { class: 'link', type: 'button', onclick: exportCsv }, t('exportCsv')) : null),
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
  if (view.backupCheck) {
    if (modalKind !== 'check' || checkShown !== JSON.stringify(view.backupCheck)) openBackupCheck();
  } else if (modalKind === 'check') {
    modalOnClose = null;
    $('modal').close();
  }
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
      r.previousFee ? h('tr', {}, h('td', {}, t('previousFee')), h('td', { class: 'num' }, `${r.previousFee} BTC`)) : null,
      h('tr', {}, h('td', {}, t('fee'), r.feeRate ? ` (${t('feeRateUsed', { rate: r.feeRate })})` : ''), h('td', { class: 'num' }, `${r.fee} BTC`)),
      h('tr', { class: 'total' }, h('td', {}, t('total')), h('td', { class: 'num' }, `${r.total} BTC`, fiat(r.total) ? h('div', { class: 'small muted' }, `≈ ${fiat(r.total)}`) : null)),
      r.credited ? h('tr', {}, h('td', {}, t('credited'), r.confirmations ? ` (${t('afterConf', { n: r.confirmations })})` : ''), h('td', { class: 'num' }, `${r.credited} BTC`)) : null,
      r.payoutFee ? h('tr', {}, h('td', {}, t('payoutFee')), h('td', { class: 'num' }, `${r.payoutFee} BTC`)) : null)),
    status,
    h('div', { class: 'row spread' }, cancelButton, confirmButton));
}

function openModal(...content) {
  modalOnClose = null;
  modalKind = null;
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
  // After making or importing one: the list again, unless its backup is to
  // be checked first.
  const after = (v) => { if (v && !v.backupCheck) openWallets(); };
  const rows = (view.wallets || []).map((w) => h('li', {},
    h('div', { class: 'grow' },
      h('div', {}, h('strong', {}, walletName(w)), w.active ? h('span', { class: 'tag vm' }, t('active')) : null,
        w.hardware === false ? h('span', { class: 'tag warn' }, t('tpmNo')) : null),
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

/** The address as a QR code, drawn here from the modules the Rust side
 *  computed: dark on white whatever the theme, with the quiet zone scanners
 *  need. */
async function openQr() {
  const qr = await act(() => invoke('receive_qr'));
  if (!qr) return;
  const scale = 6;
  const quiet = 4;
  const size = (qr.width + quiet * 2) * scale;
  const canvas = h('canvas', { width: String(size), height: String(size), class: 'qr', role: 'img', 'aria-label': qr.address });
  const ctx = canvas.getContext('2d');
  ctx.fillStyle = '#fff';
  ctx.fillRect(0, 0, size, size);
  ctx.fillStyle = '#000';
  for (let y = 0; y < qr.width; y++) {
    for (let x = 0; x < qr.width; x++) {
      if (qr.modules[y * qr.width + x] === '1') ctx.fillRect((x + quiet) * scale, (y + quiet) * scale, scale, scale);
    }
  }
  openModal(
    h('h2', {}, t('qrTitle')),
    h('div', { class: 'qr-wrap' }, canvas),
    h('p', { class: 'small muted' }, t('qrText')),
    h('p', { class: 'mono' }, qr.address),
    h('div', { class: 'row spread' }, h('span'), h('button', { type: 'button', onclick: () => $('modal').close() }, t('close'))));
}

/** A higher fee for a payment still waiting on Bitcoin: pick it, review the
 *  replacement, sign. */
function openBump(o) {
  openModal(
    h('h2', {}, t('bumpTitle')),
    h('p', { class: 'small' }, `${t(`kinds.${o.kind}`)} · ${(o.amount / 1e8).toFixed(8).replace(/\.?0+$/, '')} BTC → `, h('span', { class: 'mono' }, short(o.to))),
    h('p', { class: 'small muted' }, t('bumpText')),
    feeBlock('bump'),
    h('div', { class: 'row spread' },
      h('button', { type: 'button', onclick: () => $('modal').close() }, t('cancel')),
      h('button', { class: 'primary', type: 'button', onclick: async () => {
        const review = await act(() => invoke('prepare_bump', { txid: o.txid, feeRate: chosenFee('bump') }));
        if (review) showReview(review);
      } }, t('review'))));
  updateFeeNote('bump');
}

async function exportCsv() {
  const saved = await act(() => invoke('export_history'));
  if (saved === true) toast(t('exported'));
}

/** Asks for some words (or characters) of the backup just shown. Closing it
 *  any other way than checking leaves the wallet as it was. */
function openBackupCheck() {
  const c = view.backupCheck;
  const inputs = c.items.map((_, i) => h('input', { id: `check-${i}`, autocomplete: 'off', spellcheck: 'false' }));
  const label = ([a, b]) => (c.kind === 'words' ? t('checkWord', { n: a }) : t('checkChars', { a, b }));
  const verify = async () => {
    const result = await act(() => invoke('verify_backup', { answers: inputs.map((i) => i.value) }));
    if (result) {
      modalOnClose = null;
      $('modal').close();
      toast(t('checkOk'));
    }
  };
  openModal(
    h('h2', {}, t('checkTitle')),
    h('p', {}, t(c.kind === 'words' ? 'checkWordsText' : 'checkCharsText')),
    ...inputs.flatMap((input, i) => [h('label', { for: input.id }, label(c.items[i])), input]),
    h('div', { class: 'row spread' },
      h('button', { type: 'button', onclick: () => $('modal').close() }, t('notNow')),
      h('div', { class: 'row' },
        h('button', { type: 'button', onclick: () => act(() => invoke('backup')) }, t('showAgain')),
        h('button', { class: 'primary', type: 'button', onclick: verify }, t('check')))));
  modalKind = 'check';
  checkShown = JSON.stringify(c);
  modalOnClose = () => invoke('cancel_backup_check').then(apply).catch(showError);
  inputs[0].focus();
}

// --- settings and about ------------------------------------------------------------

function openSettings() {
  const server = h('input', { id: 'server', value: view.server, spellcheck: 'false' });
  const language = h('select', { id: 'language' }, h('option', { value: 'es' }, 'Español'), h('option', { value: 'en' }, 'English'));
  language.value = lang;
  language.addEventListener('change', () => act(() => invoke('set_language', { language: language.value })).then(openSettings));
  const currency = h('select', { id: 'fiat' }, h('option', { value: 'EUR' }, 'EUR (€)'), h('option', { value: 'USD' }, 'USD ($)'), h('option', { value: 'none' }, t('currencyNone')));
  currency.value = view.fiatChoice || 'none';
  const checkBalances = h('input', { type: 'checkbox', id: 'check-balances' });
  checkBalances.checked = !!view.checkBalances;
  checkBalances.addEventListener('change', () => act(() => invoke('set_check_balances', { on: checkBalances.checked })).then(openSettings));
  currency.addEventListener('change', () => act(() => invoke('set_fiat', { currency: currency.value })).then(openSettings));
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
    h('label', { for: 'fiat' }, t('currency')), currency,
    h('label', { class: 'check' }, checkBalances, t('checkBalances')),
    h('div', { class: 'row' }, h('button', { type: 'button', onclick: () => invoke('open_logs').catch(showError) }, t('openLogs'))),
    h('h3', {}, t('security')),
    view.hasWallet ? h('p', { class: 'small' }, `${t('tpmStatus')}: `, view.hardware === true ? t('tpmCertified') : view.hardware === false ? t('tpmNotCertified') : t('tpmUnknown')) : null,
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

$('modal').addEventListener('close', () => {
  const then = modalOnClose;
  modalOnClose = null;
  modalKind = null;
  if (then) then();
});
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
listen('notice', (event) => {
  const n = event.payload;
  toast(t(n.confirmed ? 'received' : 'receivedPending', { amount: n.amount, chain: n.chain === 'btcvm' ? 'BTCVM' : 'Bitcoin' }));
});
invoke('hello', { language: navigator.language || 'en' }).then(apply).catch(showError);
invoke('refresh').then(apply).catch(showError);
