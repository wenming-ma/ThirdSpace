const { invoke } = window.__TAURI__.core;

// Model search state
let modelsCache = null;
let modelsFetching = false;

async function loadConfig() {
    try {
        const config = await invoke('get_config');
        document.getElementById('apiKey').value = config.api_key || '';
        document.getElementById('model').value = config.model || '';
        document.getElementById('targetLanguage').value = config.target_language || 'English';
        document.getElementById('hotkey').value = config.hotkey || 'Ctrl+Alt+T';
        document.getElementById('reasoning').checked = config.reasoning_enabled !== false;
        document.getElementById('autostart').checked = config.autostart === true;
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
            reasoning_enabled: document.getElementById('reasoning').checked,
            autostart: document.getElementById('autostart').checked
        };
        await invoke('save_config', { newConfig: config });
    } catch (e) {
        console.error('Failed to save config:', e);
    }
}

function toggleModelDropdown() {
    const dropdown = document.getElementById('modelDropdown');
    const isShowing = dropdown.classList.toggle('show');
    if (isShowing) {
        // Trigger search with current input value
        const query = document.getElementById('model').value;
        searchModels(query);
    }
}

function toggleAdvanced() {
    const header = document.querySelector('.advanced-header');
    const content = document.getElementById('advancedContent');
    header.classList.toggle('expanded');
    content.classList.toggle('expanded');
}

function selectModel(model) {
    document.getElementById('model').value = model;
    document.getElementById('modelDropdown').classList.remove('show');
}

// Fetch models from OpenRouter API
async function fetchModels() {
    if (modelsCache) return modelsCache;
    if (modelsFetching) return null;

    modelsFetching = true;
    try {
        const models = await invoke('fetch_models');
        modelsCache = models;
        return models;
    } catch (e) {
        console.error('Failed to fetch models:', e);
        return null;
    } finally {
        modelsFetching = false;
    }
}

// Filter models by query
function filterModels(query, models) {
    if (!models) return [];
    const q = query.toLowerCase().trim();
    if (!q) return models.slice(0, 10);
    return models
        .filter(m => m.id.toLowerCase().includes(q) || m.name.toLowerCase().includes(q))
        .slice(0, 10);
}

// Update dropdown with filtered results
function updateDropdown(models, isLoading = false, error = null) {
    const dropdown = document.getElementById('modelDropdown');
    dropdown.innerHTML = '';

    if (isLoading) {
        dropdown.innerHTML = '<div class="model-option loading">Loading models...</div>';
        return;
    }

    if (error) {
        dropdown.innerHTML = `<div class="model-option error">${error}</div>`;
        return;
    }

    if (!models || models.length === 0) {
        dropdown.innerHTML = '<div class="model-option empty">No matching models</div>';
        return;
    }

    models.forEach(m => {
        const option = document.createElement('div');
        option.className = 'model-option';
        option.textContent = m.id;
        option.onclick = () => selectModel(m.id);
        dropdown.appendChild(option);
    });
}

// Search models with debounce
async function searchModels(query) {
    const dropdown = document.getElementById('modelDropdown');
    const apiKey = document.getElementById('apiKey').value;

    // Check if API key is configured
    if (!apiKey.trim()) {
        updateDropdown(null, false, 'Enter API key first');
        dropdown.classList.add('show');
        return;
    }

    // Show loading state
    updateDropdown(null, true);
    dropdown.classList.add('show');

    // Fetch and filter
    const models = await fetchModels();
    if (models) {
        const filtered = filterModels(query, models);
        updateDropdown(filtered);
    } else {
        updateDropdown(null, false, 'Failed to load models');
    }
}

// Close dropdown when clicking outside
document.addEventListener('click', (e) => {
    if (!e.target.closest('.model-input-wrapper')) {
        document.getElementById('modelDropdown').classList.remove('show');
    }
});

// Model search on input - immediate filtering
document.getElementById('model').addEventListener('input', (e) => {
    const query = e.target.value;
    const apiKey = document.getElementById('apiKey').value;
    const dropdown = document.getElementById('modelDropdown');

    if (!apiKey.trim()) {
        updateDropdown(null, false, 'Enter API key first');
        dropdown.classList.add('show');
        return;
    }

    // If we have cached models, filter immediately
    if (modelsCache) {
        const filtered = filterModels(query, modelsCache);
        updateDropdown(filtered);
        dropdown.classList.add('show');
    } else {
        // Fetch models first time
        searchModels(query);
    }
});

// Show dropdown on focus if there's content or API key
document.getElementById('model').addEventListener('focus', () => {
    const apiKey = document.getElementById('apiKey').value;
    if (apiKey.trim()) {
        const query = document.getElementById('model').value;
        searchModels(query);
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
