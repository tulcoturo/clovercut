'use strict';
(async () => {
  const views = [
    {id:'workspace', file:'editor-workspace', label:'Editor', title:'Your whole composition, in one workspace.', description:'Layer video, text and animated stickers on a compact multitrack timeline.', alt:'Clovercut desktop editor showing the Untitled project 10 composition, media library and multitrack timeline'},
    {id:'style', file:'text-styling', label:'Text & styling', title:'Give every word your own style.', description:'Type presets, colour and placement alongside your preview.', alt:'Clovercut default title, text style presets and typography controls'},
    {id:'giphy', file:'giphy-stickers', label:'Stickers', title:'A little movement. A lot of personality.', description:'Search GIPHY, preview stickers and place them on the timeline.', alt:'Clovercut GIPHY library with an animated sticker applied over video'},
    {id:'audio', file:'audio-library', label:'Music & sound', title:'Find the sound your story needs.', description:'Separate music and sound effects, with track previews and licence information.', alt:'Clovercut YouTube Audio Library music list with sound effect categories and audio controls'},
    {id:'transitions', file:'transitions-shelf', label:'Transitions', title:'Find your way into the next scene.', description:'Preview transitions and keep your favourites within reach.', alt:'Clovercut transition library with preview thumbnails and favourite stars'},
    {id:'export', file:'export-dialog', label:'Export', title:'Finish with the settings you choose.', description:'Video, audio and output controls together in one export dialog.', alt:'Clovercut video export dialog showing output, encoding and audio settings'},
    {id:'start', file:'start-screen', label:'Start screen', title:'A clear place to start.', description:'Create a project, open an edit or find your next step.', alt:'Clovercut start screen with new project, open project and learning resources'}
  ];
  const source = view => `site_assets/${view.file}.png?v=20261027`;
  const counter = index => `${String(index+1).padStart(2,'0')} / ${String(views.length).padStart(2,'0')}`;
  const findIndex = id => views.findIndex(view => view.id === id);
  let selected = 0;
  let viewing = 0;
  let returnFocus = null;
  const tabs = [...document.querySelectorAll('[role="tab"][data-view]')];
  const stage = document.getElementById('editor-stage');
  const preview = document.getElementById('preview-image');
  const stageButton = stage.querySelector('.screenshot-button');
  const viewer = document.getElementById('screenshot-viewer');
  const viewerScroll = document.getElementById('viewer-scroll');
  const viewerImage = document.getElementById('viewer-image');
  const zoom = document.getElementById('viewer-zoom');

  function selectView(index, focus=false) {
    if (index < 0 || index >= views.length) return;
    selected = index;
    const view = views[index];
    tabs.forEach(tab => {
      const active = tab.dataset.view === view.id;
      tab.setAttribute('aria-selected',String(active));
      tab.tabIndex = active ? 0 : -1;
      if (active && focus) { tab.focus(); tab.scrollIntoView({block:'nearest',inline:'nearest'}); }
    });
    stage.setAttribute('aria-labelledby',`tab-${view.id}`);
    stageButton.dataset.openView = view.id;
    stageButton.setAttribute('aria-label',`Enlarge ${view.label} screenshot`);
    preview.src = source(view);
    preview.alt = view.alt;
    document.getElementById('view-counter').textContent = counter(index);
    document.getElementById('view-title').textContent = view.title;
    document.getElementById('view-description').textContent = view.description;
  }

  tabs.forEach(tab => tab.addEventListener('click',() => selectView(findIndex(tab.dataset.view))));
  document.querySelector('.view-tabs').addEventListener('keydown',event => {
    let next;
    if (event.key === 'ArrowRight') next = (selected+1)%views.length;
    else if (event.key === 'ArrowLeft') next = (selected+views.length-1)%views.length;
    else if (event.key === 'Home') next = 0;
    else if (event.key === 'End') next = views.length-1;
    else return;
    event.preventDefault();
    selectView(next,true);
  });

  function setZoom(actual) {
    viewerScroll.classList.toggle('is-actual',actual);
    zoom.setAttribute('aria-pressed',String(actual));
    zoom.textContent = actual ? 'Fit to screen' : 'Actual size';
    viewerScroll.scrollTop = 0;
    viewerScroll.scrollLeft = 0;
  }

  function renderViewer(index) {
    viewing = (index+views.length)%views.length;
    const view = views[viewing];
    document.getElementById('viewer-title').textContent = view.label;
    document.getElementById('viewer-counter').textContent = counter(viewing);
    document.getElementById('viewer-description').textContent = view.description;
    viewerImage.src = source(view);
    viewerImage.alt = view.alt;
    setZoom(false);
  }

  function openViewer(id) {
    const index = findIndex(id);
    if (index < 0) return;
    returnFocus = document.activeElement;
    renderViewer(index);
    document.body.style.overflow = 'hidden';
    viewer.showModal();
    document.getElementById('viewer-close').focus();
  }

  document.querySelectorAll('[data-open-view]').forEach(button => button.addEventListener('click',() => openViewer(button.dataset.openView)));
  document.getElementById('expand-screenshot').addEventListener('click',() => openViewer(views[selected].id));
  document.getElementById('viewer-prev').addEventListener('click',() => renderViewer(viewing-1));
  document.getElementById('viewer-next').addEventListener('click',() => renderViewer(viewing+1));
  document.getElementById('viewer-close').addEventListener('click',() => viewer.close());
  zoom.addEventListener('click',() => setZoom(zoom.getAttribute('aria-pressed') !== 'true'));
  viewerImage.addEventListener('click',() => setZoom(zoom.getAttribute('aria-pressed') !== 'true'));
  viewer.addEventListener('click',event => { if (event.target === viewer) viewer.close(); });
  viewer.addEventListener('keydown',event => {
    if (event.altKey || event.ctrlKey || event.metaKey) return;
    if (viewerScroll.classList.contains('is-actual') && viewerScroll.contains(event.target)) return;
    if (event.key === 'ArrowLeft' || event.key === 'ArrowRight') {
      event.preventDefault();
      renderViewer(viewing+(event.key === 'ArrowRight'?1:-1));
    }
  });
  viewer.addEventListener('close',() => {
    document.body.style.overflow = '';
    setZoom(false);
    if (returnFocus?.isConnected) returnFocus.focus({preventScroll:true});
  });

  const ua = navigator.userAgent || '';
  const platform = navigator.userAgentData?.platform || navigator.platform || '';
  const mobile = /Android|iPhone|iPad|iPod/i.test(ua) || (/Mac/i.test(platform) && navigator.maxTouchPoints > 1);
  const detected = mobile ? null : /Win/i.test(platform+' '+ua) ? 'windows' : /Mac/i.test(platform+' '+ua) ? 'macos' : /Linux/i.test(platform+' '+ua) ? 'linux' : null;
  if (detected) {
    const card = document.querySelector(`[data-platform="${detected}"]`);
    card.dataset.detected = 'true';
    card.querySelector('.platform-badge').hidden = false;
    const button = document.getElementById('smart-download-btn');
    const label = document.getElementById('smart-download-label');
    // Windows on ARM often advertises x64 in the legacy user-agent string.
    // Ask for the OS architecture, rather than the browser binary's branding.
    let arch = /aarch64|arm64/i.test(ua) ? 'aarch64' : null;
    try {
      const hints = await navigator.userAgentData?.getHighEntropyValues(['architecture', 'bitness']);
      if (hints?.bitness === '64') {
        if (hints.architecture === 'arm') arch = 'aarch64';
        else if (hints.architecture === 'x86') arch = 'x86_64';
      }
    } catch { /* Browsers may withhold high-entropy hints. Keep both choices. */ }
    if (detected === 'linux' && !arch && /x86_64|amd64/i.test(ua)) arch = 'x86_64';
    if (detected === 'macos' || !arch) {
      button.href = '#downloads';
      label.textContent = detected === 'macos' ? 'Choose your Mac version' : `Choose your ${detected === 'windows' ? 'Windows' : 'Linux'} version`;
    } else {
      const file = detected === 'windows' ? `windows-${arch}-setup.exe` : `${arch}.AppImage`;
      const href = `https://github.com/tulcoturo/clovercut/releases/download/v0.2.1/Clovercut-0.2.1-${file}`;
      const architecture = arch === 'aarch64' ? 'ARM64' : 'x64';
      button.href = href;
      label.textContent = `Download for ${detected === 'windows' ? 'Windows' : 'Linux'} ${architecture}`;
      const primary = card.querySelector('.button');
      primary.href = href;
      primary.firstChild.textContent = `Download for ${architecture}`;
      // The explicit architecture links remain available regardless of detection.
    }
  }
})();
