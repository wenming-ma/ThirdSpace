const { invoke } = window.__TAURI__.core;

async function loadConfig() {
    try {
        const config = await invoke('get_config');
        document.getElementById('apiKey').value = config.api_key || '';
        document.getElementById('model').value = config.model || '';
        document.getElementById('targetLanguage').value = config.target_language || 'English';
        document.getElementById('hotkey').value = config.hotkey || 'Ctrl+Alt+T';
        document.getElementById('reasoning').checked = config.reasoning_enabled !== false;
    } catch (e) {
        console.error('Failed to load config:', e);
    }
}

async function save() {
    try {
        const config = {
            api_key: document.getElementById('apiKey').value,
            model: document.getElementById('model').value,
            target_language: document.getElementById('targetLanguage').value,
            hotkey: document.getElementById('hotkey').value,
            reasoning_enabled: document.getElementById('reasoning').checked
        };
        await invoke('save_config', { newConfig: config });
    } catch (e) {
        console.error('Failed to save config:', e);
    }
}

function toggleModelDropdown() {
    const dropdown = document.getElementById('modelDropdown');
    dropdown.classList.toggle('show');
}

function selectModel(model) {
    document.getElementById('model').value = model;
    document.getElementById('modelDropdown').classList.remove('show');
}

// Close dropdown when clicking outside
document.addEventListener('click', (e) => {
    if (!e.target.closest('.model-input-wrapper')) {
        document.getElementById('modelDropdown').classList.remove('show');
    }
});

// Handle external links
document.querySelectorAll('.tooltip-text a').forEach(link => {
    link.addEventListener('click', async (e) => {
        e.preventDefault();
        const url = link.getAttribute('href');
        if (window.__TAURI__?.shell) {
            await window.__TAURI__.shell.open(url);
        }
    });
});

// Hotkey recording
const hotkeyInput = document.getElementById('hotkey');
let isRecording = false;

hotkeyInput.addEventListener('focus', async () => {
    // Pause the global hotkey so we can capture any key combination
    try {
        await invoke('pause_hotkey');
    } catch (e) {
        console.error('Failed to pause hotkey:', e);
    }
    isRecording = true;
    hotkeyInput.value = '';
    hotkeyInput.placeholder = 'Press keys...';
});

hotkeyInput.addEventListener('blur', async () => {
    isRecording = false;
    // Resume the global hotkey
    try {
        await invoke('resume_hotkey');
    } catch (e) {
        console.error('Failed to resume hotkey:', e);
    }
    if (!hotkeyInput.value) {
        hotkeyInput.placeholder = 'Click and press keys...';
    }
});

// Use capture phase to intercept browser shortcuts (Ctrl+W, Ctrl+N, etc.) before they're handled
document.addEventListener('keydown', (e) => {
    if (!isRecording) return;

    // Prevent browser/system shortcuts immediately
    e.preventDefault();
    e.stopPropagation();

    // Ignore standalone modifier keys
    if (['Control', 'Alt', 'Shift', 'Meta'].includes(e.key)) {
        return;
    }

    const parts = [];
    if (e.ctrlKey) parts.push('Ctrl');
    if (e.altKey) parts.push('Alt');
    if (e.shiftKey) parts.push('Shift');
    if (e.metaKey) parts.push('Win');

    // Get key name
    let key = e.key;
    if (key === ' ') key = 'Space';
    else if (key.length === 1) key = key.toUpperCase();
    else if (key.startsWith('Arrow')) key = key.replace('Arrow', '');

    parts.push(key);

    hotkeyInput.value = parts.join('+');
    hotkeyInput.blur();
}, { capture: true });

// Load config on startup
loadConfig();
