var model = 'deepseek-chat';

async function init() {
    var r = await fetch('/api/status');
    var d = await r.json();
    var s = document.getElementById('status');
    s.innerHTML = d.api_key ? 'Online' : 'No key';
    s.className = 'status' + (d.api_key ? ' online' : '');
    addTool('shell, read_file, write_file, grep, git, web_fetch');
    addConfig('Model', d.model);
    addConfig('Version', d.version);
    addConfig('API Key', d.api_key ? 'set' : 'missing');
}

function addTool(n) { document.getElementById('tool-list').innerHTML = n.split(',').map(function(t) { return '<span>' + t.trim() + '</span>'; }).join(''); }

function addConfig(k, v) { document.getElementById('config-info').innerHTML += '<div><b>' + k + ':</b> ' + v + '</div>'; }

async function send() {
    var input = document.getElementById('prompt');
    var prompt = input.value.trim();
    if (!prompt) return;
    input.value = '';
    addMsg('user', prompt);
    addMsg('assistant', 'Thinking...', 'thinking');
    try {
        var r = await fetch('/api/chat', { method: 'POST', body: JSON.stringify({prompt: prompt, model: model}) });
        var d = await r.json();
        var msgs = document.getElementById('messages');
        var last = msgs.querySelector('.msg.assistant:last-child');
        if (last && last.classList.contains('thinking')) last.remove();
        addMsg('assistant', d.response || d.error || 'No response');
    } catch(e) {
        addMsg('assistant', 'Error: ' + e);
    }
}

function addMsg(role, content, cls) {
    var div = document.createElement('div');
    div.className = 'msg ' + role + (cls ? ' ' + cls : '');
    div.innerHTML = '<div class="role">' + (role === 'user' ? 'You' : 'Hermes') + '</div><div class="content">' + escHtml(content) + '</div>';
    document.getElementById('messages').appendChild(div);
    div.scrollIntoView({behavior:'smooth'});
}

function clearChat() { document.getElementById('messages').innerHTML = ''; }

function switchTab(name) {
    var tabs = document.querySelectorAll('.tab');
    for (var i = 0; i < tabs.length; i++) tabs[i].classList.remove('active');
    var links = document.querySelectorAll('nav a');
    for (var i = 0; i < links.length; i++) links[i].classList.remove('active');
    document.getElementById('tab-' + name).classList.add('active');
    document.querySelector('nav a[onclick*="' + name + '"]').classList.add('active');
}

document.getElementById('model-select').onchange = function() { model = this.value; };

function escHtml(s) { return s.replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/\n/g,'<br>'); }

init();
