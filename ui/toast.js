const icons = {
    success: '<svg viewBox="0 0 24 24"><path d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z"/></svg>',
    error: '<svg viewBox="0 0 24 24"><path d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"/></svg>',
    processing: '<svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="10" fill="none" stroke="currentColor" stroke-width="2" stroke-dasharray="31.4" stroke-linecap="round"><animateTransform attributeName="transform" type="rotate" from="0 12 12" to="360 12 12" dur="1s" repeatCount="indefinite"/></circle></svg>'
};

const labels = {
    success: 'Done',
    error: 'Error',
    processing: 'Translating...'
};

function update(kind, title) {
    const toast = document.getElementById('toast');
    const icon = document.getElementById('icon');
    const text = document.getElementById('text');

    toast.className = 'toast ' + kind;
    icon.innerHTML = icons[kind] || icons.success;
    text.textContent = title || labels[kind] || kind;
}

// Read initial state from URL params
const params = new URLSearchParams(window.location.search);
const initialKind = params.get('kind');
const initialTitle = params.get('title');
if (initialKind) {
    update(initialKind, initialTitle || '');
}

// Listen for updates from Tauri
if (window.__TAURI__) {
    window.__TAURI__.event.listen('update-toast', (event) => {
        update(event.payload.kind, event.payload.title);
    });
}
