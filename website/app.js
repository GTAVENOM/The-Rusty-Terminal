// --- Rusty Terminal Website Production App Script ---
// Features: Immediate Top Scroll Enforcement, Dot-Free Clean Canvas Grid Background, Scroll Reveal Observer

// 1. Force Page Scroll to Top Hero Section Immediately on Script Load
if ('scrollRestoration' in history) {
  history.scrollRestoration = 'manual';
}
window.scrollTo(0, 0);
window.onbeforeunload = function () {
  window.scrollTo(0, 0);
};

document.addEventListener('DOMContentLoaded', () => {
  window.scrollTo(0, 0);

  // 2. Dot-Free Clean Tech Box Grid Canvas Background
  initCleanTechGridCanvas();

  // 3. Scroll Reveal Observer Controller
  initScrollRevealObserver();

  // 4. Tab Switching for Install Commands
  const tabBtns = document.querySelectorAll('.tab-btn');
  const installCode = document.getElementById('install-code');

  const installSnippets = {
    powershell: 'irm https://rustyterminal.vercel.app/install.ps1 | iex',
    cmd: 'curl -fsSL https://rustyterminal.vercel.app/install.cmd -o install.cmd && install.cmd',
    macos: 'curl -fsSL https://rustyterminal.vercel.app/install.sh | bash'
  };

  tabBtns.forEach(btn => {
    btn.addEventListener('click', () => {
      tabBtns.forEach(b => b.classList.remove('active'));
      btn.classList.add('active');
      const tab = btn.getAttribute('data-tab');
      if (installSnippets[tab]) {
        installCode.textContent = installSnippets[tab];
      }
    });
  });

  // 5. Copy to Clipboard Action
  const copyBtn = document.getElementById('copy-btn');
  const copyText = document.getElementById('copy-text');

  copyBtn.addEventListener('click', () => {
    const text = installCode.textContent;
    navigator.clipboard.writeText(text).then(() => {
      copyText.textContent = 'Copied! ✅';
      copyBtn.style.background = 'rgba(34, 197, 94, 0.25)';
      setTimeout(() => {
        copyText.textContent = 'Copy';
        copyBtn.style.background = '';
      }, 2000);
    });
  });

  // 6. Live Neural AI Terminal Simulator Engine
  const demoInput = document.getElementById('demo-input');
  const outputArea = document.getElementById('output-area');
  const chips = document.querySelectorAll('.chip');

  function parseDynamicIntent(query) {
    const lower = query.toLowerCase().trim();

    // Directory Navigation & Disambiguation
    if (lower === 'go to kt' || lower === 'cd kt') {
      return {
        rendered: '🔍 Multiple matching targets found:\n  [1] cd kt/ (Navigate to kt/ directory)\n  [2] cd kt_backend/ (Navigate to kt_backend/ directory)\nSelect [1-2]: 1\n✨ Executing: cd kt/',
        tier: 'Multi-Option Disambiguation',
        tierClass: 'tier-1',
        desc: 'Disambiguates ambiguous targets into a numbered choice selection.'
      };
    }

    if (lower.startsWith('go to ') || lower.startsWith('cd ') || lower.startsWith('navigate to ') || lower.startsWith('take me to ')) {
      const target = lower.replace(/^(go to|cd|navigate to|take me to)\s+/, '').trim();
      const cleanTarget = target.endsWith('/') ? target : `${target}/`;
      return {
        rendered: `✨ Executing: cd ${cleanTarget}`,
        tier: 'Tier 1 (Read-Only)',
        tierClass: 'tier-1',
        desc: `Navigates shell working directory to ${cleanTarget}`
      };
    }

    // Kubernetes & Cloud Pod Scaling
    if (lower.includes('pod') || lower.includes('replica') || lower.includes('scale')) {
      const countMatch = lower.match(/\b(\d+)\b/);
      const replicas = countMatch ? countMatch[1] : '5';
      return {
        rendered: `✨ Executing: kubectl scale deployment/web-app --replicas=${replicas}`,
        tier: 'Tier 2 (Idempotent)',
        tierClass: 'tier-1',
        desc: `Scales Kubernetes deployment replicas to ${replicas}.`
      };
    }

    // Permissions & Security (chmod / icacls)
    if (lower.includes('permission') || lower.includes('chmod') || lower.includes('grant') || lower.includes('access')) {
      const fileMatch = lower.match(/([a-zA-Z0-9_.-]+\.[a-zA-Z0-9]+)/);
      const file = fileMatch ? fileMatch[1] : 'coding.c';
      return {
        rendered: `✨ Executing: icacls ${file} /grant Everyone:F`,
        tier: 'Tier 2 (Idempotent)',
        tierClass: 'tier-1',
        desc: `Grants full access permissions for file '${file}'.`
      };
    }

    // Package Management (npm / pip / cargo)
    if (lower.includes('npm install') || lower.includes('install express') || (lower.includes('install') && lower.includes('npm'))) {
      const pkg = lower.includes('express') ? 'express' : lower.replace(/.*(npm install|install)\s+/, '').trim() || 'package';
      return {
        rendered: `✨ Executing: npm install ${pkg}`,
        tier: 'Tier 2 (Idempotent)',
        tierClass: 'tier-1',
        desc: `Installs Node.js package '${pkg}' into local project dependencies.`
      };
    }

    if (lower.includes('pip install') || (lower.includes('install') && lower.includes('python'))) {
      const pkg = lower.replace(/.*(pip install|install)\s+/, '').trim() || 'requests';
      return {
        rendered: `✨ Executing: pip install ${pkg}`,
        tier: 'Tier 2 (Idempotent)',
        tierClass: 'tier-1',
        desc: `Installs Python module '${pkg}' via pip.`
      };
    }

    if (lower.includes('cargo add') || (lower.includes('add') && lower.includes('cargo'))) {
      const crate = lower.replace(/.*(cargo add|add)\s+/, '').trim() || 'tokio';
      return {
        rendered: `✨ Executing: cargo add ${crate}`,
        tier: 'Tier 2 (Idempotent)',
        tierClass: 'tier-1',
        desc: `Adds Rust crate '${crate}' to Cargo.toml dependencies.`
      };
    }

    // Docker Image Pulling & Container Operations
    if (lower.includes('pull') || lower.includes('docker pull') || (lower.includes('image') && lower.includes('ubuntu'))) {
      let image = 'ubuntu:latest';
      if (lower.includes('redis')) image = 'redis:latest';
      else if (lower.includes('postgres')) image = 'postgres:latest';
      else if (lower.includes('nginx')) image = 'nginx:alpine';
      else if (lower.includes('node')) image = 'node:alpine';
      else if (lower.includes('ubuntu')) image = 'ubuntu:latest';

      return {
        rendered: `✨ Executing: docker pull ${image}`,
        tier: 'Tier 2 (Idempotent)',
        tierClass: 'tier-1',
        desc: `Pulls container image ${image} from official container registry.`
      };
    }

    if (lower.includes('docker ps') || lower.includes('running containers') || lower.includes('list containers')) {
      return {
        rendered: '✨ Executing: docker ps -a',
        tier: 'Tier 1 (Read-Only)',
        tierClass: 'tier-1',
        desc: 'Lists active and stopped container instances.'
      };
    }

    // Git History & Commits
    if (lower.includes('git commit') || lower.includes('git log') || lower.includes('commits')) {
      const match = lower.match(/\b(\d+)\b/);
      const count = match ? match[1] : '5';
      return {
        rendered: `✨ Executing: git log -n ${count} --oneline`,
        tier: 'Tier 1 (Read-Only)',
        tierClass: 'tier-1',
        desc: `Retrieves last ${count} commit log messages in concise single-line format.`
      };
    }

    if (lower.includes('git status') || lower.includes('what changed')) {
      return {
        rendered: '✨ Executing: git status',
        tier: 'Tier 1 (Read-Only)',
        tierClass: 'tier-1',
        desc: 'Checks working directory and staging area status.'
      };
    }

    // Port & Process Lookup
    if (lower.includes('port')) {
      const match = lower.match(/\b(\d{2,5})\b/);
      const port = match ? match[1] : '8080';
      return {
        rendered: `✨ Executing: Get-NetTCPConnection -LocalPort ${port}`,
        tier: 'Tier 1 (Read-Only)',
        tierClass: 'tier-1',
        desc: `Searches active network sockets for process bound to port ${port}.`
      };
    }

    // Code Generation Agent
    if (lower.includes('script') || lower.includes('i want a') || lower.includes('python') || lower.includes('code')) {
      const fileName = lower.includes('rename') ? 'rename_files.py' : 'script_1.py';
      return {
        rendered: `📄 Code generated and written to: .rusty_scratch/${fileName} (Not executed)`,
        tier: 'Code Generation Agent',
        tierClass: 'tier-1',
        desc: 'Generates requested code directly to file in scratch directory with numeric collision protection.'
      };
    }

    // Bounded Error Help Engine
    if (lower.includes('modulenotfounderror') || lower.includes('error help') || lower.includes('not found')) {
      return {
        rendered: '💡 [Module / Package Not Found] Python module "requests" is missing.\n   Suggested Fix: pip install requests',
        tier: 'Bounded Error Help',
        tierClass: 'tier-1',
        desc: 'Pattern matches non-zero exit codes & stderr to surface instant 1-line fixes.'
      };
    }

    // Tier 3 Destructive Commands (Blocked)
    if (lower.includes('rm -rf') || lower.includes('git push --force') || lower.includes('kubectl delete') || lower.includes('destructive') || lower.includes('drop database') || lower.includes('kill process')) {
      const cmd = lower.includes('rm') ? 'rm -rf /' : lower.includes('drop') ? 'DROP DATABASE production;' : lower.includes('kill') ? 'Stop-Process -Id 1234 -Force' : 'git push --force';
      return {
        rendered: `🛑 [DESTRUCTIVE — reference only]\n   Reference command: ${cmd}\n   (Structurally incapable of auto-insertion into live shell line. Manual typing required.)`,
        tier: 'Tier 3 (Destructive - BLOCKED)',
        tierClass: 'tier-3',
        desc: 'Destructive commands are displayed as read-only reference and blocked from shell auto-insertion.'
      };
    }

    // Dynamic AI Translation Fallback
    let generatedCmd = `Get-ChildItem -Path . -Filter "*${query}*"`;
    if (lower.includes('find') || lower.includes('search')) {
      generatedCmd = `Get-ChildItem -Path . -Recurse -Filter "*${query.replace(/.*(find|search)\s+/, '')}*"`;
    } else if (lower.includes('show') || lower.includes('list')) {
      generatedCmd = `Get-Process | Where-Object {$_.ProcessName -like "*${query.replace(/.*(show|list)\s+/, '')}*"}`;
    }

    return {
      rendered: `✨ Executing: ${generatedCmd}`,
      tier: 'Tier 1 (Read-Only)',
      tierClass: 'tier-1',
      desc: `Dynamic AI Shell Translation for '${query}'`
    };
  }

  function processQuery(query) {
    if (!query.trim()) return;

    const result = parseDynamicIntent(query);

    const card = document.createElement('div');
    card.className = 'output-card';
    card.innerHTML = `
      <div class="line"><strong>Prompt:</strong> "${query}"</div>
      <span class="tier-badge ${result.tierClass}">${result.tier}</span>
      <div class="rendered-cmd">${result.rendered}</div>
      <div class="line comment"># ${result.desc}</div>
    `;
    outputArea.appendChild(card);
    outputArea.scrollTop = outputArea.scrollHeight;
  }

  if (demoInput) {
    demoInput.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') {
        processQuery(demoInput.value);
        demoInput.value = '';
      }
    });
  }

  chips.forEach(chip => {
    chip.addEventListener('click', () => {
      const prompt = chip.getAttribute('data-prompt');
      if (demoInput) demoInput.value = prompt;
      processQuery(prompt);
      if (demoInput) demoInput.value = '';
    });
  });
});

// --- Dot-Free Clean Tech Network Grid Canvas Background ---
function initCleanTechGridCanvas() {
  const canvas = document.getElementById('bg-grid-canvas');
  if (!canvas) return;

  const ctx = canvas.getContext('2d');
  let width = (canvas.width = window.innerWidth);
  let height = (canvas.height = window.innerHeight);

  window.addEventListener('resize', () => {
    width = canvas.width = window.innerWidth;
    height = canvas.height = window.innerHeight;
  });

  const gridSize = 60;
  let offset = 0;

  function draw() {
    ctx.clearRect(0, 0, width, height);

    // Draw Moving Clean Network Grid Lines (Zero Particle Dots)
    ctx.strokeStyle = 'rgba(0, 242, 254, 0.05)';
    ctx.lineWidth = 1;

    offset = (offset + 0.2) % gridSize;

    for (let x = offset; x < width; x += gridSize) {
      ctx.beginPath();
      ctx.moveTo(x, 0);
      ctx.lineTo(x, height);
      ctx.stroke();
    }

    for (let y = offset; y < height; y += gridSize) {
      ctx.beginPath();
      ctx.moveTo(0, y);
      ctx.lineTo(width, y);
      ctx.stroke();
    }

    requestAnimationFrame(draw);
  }

  draw();
}

// --- Scroll Reveal Observer Controller ---
function initScrollRevealObserver() {
  const reveals = document.querySelectorAll('.reveal');
  if (!reveals.length) return;

  const observer = new IntersectionObserver(
    (entries) => {
      entries.forEach((entry) => {
        if (entry.isIntersecting) {
          entry.target.classList.add('active');
        }
      });
    },
    {
      threshold: 0.1,
      rootMargin: '0px 0px -40px 0px'
    }
  );

  reveals.forEach((el) => observer.observe(el));
}
