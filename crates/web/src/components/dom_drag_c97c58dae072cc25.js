// Sortable owns transient pointer/touch DOM movement; Rust owns canonical order.
// Restore Leptos' DOM before updating its keyed lists (including comment markers).
export function install_board_drag(root, Sortable, start, finish) {
  const instances = new Map();
  let origin = null, active = false, canceled = false, suppressClick = false;
  const decorative = el => {
    el.inert = true;
    el.setAttribute('aria-hidden', 'true');
    for (const node of [el, ...el.querySelectorAll('[id], [data-motion-key], [data-ticket-id]')]) {
      node.removeAttribute('id');
      node.removeAttribute('data-motion-key');
      node.removeAttribute('data-ticket-id');
    }
  };
  const reduced = () => document.documentElement.dataset.motion === 'reduce' || matchMedia('(prefers-reduced-motion: reduce)').matches;
  const manual = list => list.dataset.order !== 'sorted';
  const blocked = () => root.dataset.busy === 'true' || root.dataset.moving === 'true';
  const restore = () => {
    origin?.placeholder?.remove();
    if (origin?.parent.isConnected) origin.parent.insertBefore(origin.item, origin.next);
  };
  const end = event => {
    if (!active) { origin = null; return; }
    const item = event.item, destination = item.parentElement;
    const before = item.nextElementSibling?.dataset.ticketId || 0;
    const same = destination === origin.parent;
    const unchanged = same && item.nextSibling === origin.next;
    const status = destination.dataset.status;
    const ticket = Number(item.dataset.ticketId);
    restore();
    active = false;
    // Keep the pointer flag until Rust has flushed the restored/canonical lists,
    // so the shared motion observer never animates Sortable's temporary order.
    queueMicrotask(() => { delete root.dataset.pointerDrag; });
    origin = null;
    finish(ticket, status, Number(before), canceled || unchanged || (same && !manual(destination)));
  };
  const sync = () => {
    for (const [list, instance] of instances) {
      if (!root.contains(list)) { instance.destroy(); instances.delete(list); }
      else instance.option('sort', manual(list));
    }
    for (const list of root.querySelectorAll('.column-cards')) {
      if (instances.has(list)) continue;
      const styles = getComputedStyle(root);
      const instance = new Sortable(list, {
        group: 'tickets-board', draggable: '.api-ticket', dataIdAttr: 'data-ticket-id',
        sort: manual(list), direction: 'vertical',
        forceFallback: true, fallbackOnBody: true, fallbackTolerance: 5,
        delay: 180, delayOnTouchOnly: true, touchStartThreshold: 5,
        animation: reduced() ? 0 : parseFloat(styles.getPropertyValue('--duration-move')),
        easing: styles.getPropertyValue('--ease-move').trim(),
        ghostClass: 'board-drop-placeholder', chosenClass: 'board-drag-chosen',
        fallbackClass: 'board-drag-preview',
        scroll: true, bubbleScroll: true, forceAutoScrollFallback: true,
        scrollSensitivity: 48, scrollSpeed: 12, emptyInsertThreshold: 32,
        preventOnFilter: false,
        filter: event => blocked() || !!event.target?.closest?.('button, input, textarea, select, [role="combobox"], [contenteditable="true"], a:not(.card-title)'),
        onChoose: event => {
          origin = { item: event.item, parent: event.item.parentNode, next: event.item.nextSibling };
          canceled = false;
        },
        onStart: event => {
          active = true;
          suppressClick = true;
          root.dataset.pointerDrag = 'true';
          // Keep a separate source slot: Sortable moves the real item to show
          // the destination indicator, then we restore it before Rust updates.
          const placeholder = event.item.cloneNode(true);
          placeholder.classList.remove('api-ticket', 'board-drag-chosen', 'board-drop-placeholder', 'drag-over');
          placeholder.classList.add('board-origin-placeholder');
          placeholder.style.cssText = '';
          decorative(placeholder);
          origin.placeholder = placeholder;
          origin.parent.insertBefore(placeholder, event.item);
          // Clones are decorative: no duplicate focus stops or accessible IDs.
          const preview = document.querySelector('.board-drag-preview');
          if (preview) {
            decorative(preview);
            preview.style.opacity = '1';
          }
          for (const el of root.querySelectorAll('.api-ticket')) el.getAnimations().forEach(animation => animation.cancel());
          start(Number(event.item.dataset.ticketId));
        },
        onMove: event => !canceled && (event.to !== origin?.parent || manual(event.to)),
        onEnd: end,
      });
      instances.set(list, instance);
    }
  };
  // Latch belongs to the board, not a card node that Rust may replace on drop.
  const click = event => {
    if (suppressClick && (root.contains(event.target) || event.target.closest?.('.board-drag-preview'))) {
      event.preventDefault();
      event.stopImmediatePropagation();
    }
  };
  const pointer = () => { if (!active) suppressClick = false; };
  const cancel = () => {
    if (!active) return;
    canceled = true;
    // End via the sensor's normal public DOM event path; no private Sortable API.
    document.dispatchEvent(new Event('pointerup', { bubbles: true }));
    document.dispatchEvent(new Event('mouseup', { bubbles: true }));
  };
  const key = event => {
    if (!active) {
      // A deliberate keyboard activation is a new gesture, like pointerdown.
      if (event.key === 'Enter' || event.key === ' ') suppressClick = false;
      return;
    }
    // Keyboard and pointer gestures must not mutate the same draft together.
    if (event.key === 'Escape') { event.preventDefault(); cancel(); }
    event.stopImmediatePropagation();
  };
  const release = event => {
    if (!active) return;
    const point = event.changedTouches?.[0] || event;
    if (typeof point.clientX !== 'number') return;
    const column = document.elementFromPoint(point.clientX, point.clientY)?.closest('.board-column');
    if (!column || !root.contains(column)) canceled = true;
  };
  const interrupted = event => { if (event.type !== 'visibilitychange' || document.hidden) cancel(); };
  const observer = new MutationObserver(sync);
  observer.observe(root, { childList: true, subtree: true, attributes: true, attributeFilter: ['data-order'] });
  const preference = new MutationObserver(() => {
    for (const instance of instances.values()) instance.option('animation', reduced() ? 0 : parseFloat(getComputedStyle(root).getPropertyValue('--duration-move')));
  });
  preference.observe(document.documentElement, { attributes: true, attributeFilter: ['data-motion'] });
  const media = matchMedia('(prefers-reduced-motion: reduce)');
  const motion = () => { for (const instance of instances.values()) instance.option('animation', reduced() ? 0 : parseFloat(getComputedStyle(root).getPropertyValue('--duration-move'))); };
  media.addEventListener('change', motion);
  for (const type of ['pointerup', 'mouseup', 'touchend']) document.addEventListener(type, release, true);
  window.addEventListener('click', click, true);
  document.addEventListener('pointerdown', pointer, true);
  document.addEventListener('keydown', key, true);
  document.addEventListener('pointercancel', cancel, true);
  document.addEventListener('touchcancel', cancel, true);
  document.addEventListener('visibilitychange', interrupted);
  window.addEventListener('blur', interrupted);
  sync();
  return () => {
    observer.disconnect(); preference.disconnect();
    media.removeEventListener('change', motion);
    for (const type of ['pointerup', 'mouseup', 'touchend']) document.removeEventListener(type, release, true);
    window.removeEventListener('click', click, true);
    document.removeEventListener('pointerdown', pointer, true);
    document.removeEventListener('keydown', key, true);
    document.removeEventListener('pointercancel', cancel, true);
    document.removeEventListener('touchcancel', cancel, true);
    document.removeEventListener('visibilitychange', interrupted);
    window.removeEventListener('blur', interrupted);
    if (active) { canceled = true; restore(); active = false; }
    for (const instance of instances.values()) instance.destroy();
    instances.clear();
    delete root.dataset.pointerDrag;
  };
}

// Versioned with drag: legacy dom.js remains byte-for-byte cache compatible.
export function install_motion() {
  const reduced = { get matches() { return document.documentElement.dataset.motion === 'reduce' || matchMedia('(prefers-reduced-motion: reduce)').matches; } };
  let records = new Map(), frame = 0, keyboard = false, replay = false, route = location.pathname;
  const selector = '[data-motion-key]';
  const keydown = () => { keyboard = true; };
  const pointerdown = () => { keyboard = false; };
  const sample = () => {
    frame = 0;
    if (document.querySelector('.api-board[data-pointer-drag="true"]')) { records.clear(); return; }
    const next = new Map();
    if (route !== location.pathname) { records.clear(); route = location.pathname; }
    document.querySelectorAll(selector).forEach(el => {
      if (el.closest('[data-motion-ghost]')) return;
      const key = el.dataset.motionKey, rect = el.getBoundingClientRect();
      if (!rect.width || !rect.height) return;
      const old = records.get(key);
      if (!keyboard && !reduced.matches && old) {
        const x = old.rect.x - rect.x, y = old.rect.y - rect.y;
        if (Math.abs(x) + Math.abs(y) > 1 && Math.abs(y) < innerHeight) {
          el.animate([{transform:`translate(${x}px, ${y}px)`}, {transform:'none'}], {duration:200,easing:'cubic-bezier(.77,0,.175,1)'});
        }
      } else if (!old && records.size && !keyboard) {
        el.animate([{opacity:0,transform:reduced.matches?'none':'translateY(4px)'},{opacity:1,transform:'none'}], {duration:160,easing:'cubic-bezier(.23,1,.32,1)'});
      }
      next.set(key,{rect,clone:el.cloneNode(true)});
    });
    if (!keyboard) records.forEach((old,key) => {
      if (next.has(key)) return;
      const el=old.clone, r=old.rect;
      el.removeAttribute('id'); el.removeAttribute('data-motion-key');
      el.querySelectorAll('[id]').forEach(n=>n.removeAttribute('id'));
      el.dataset.motionGhost=''; el.inert=true; el.setAttribute('aria-hidden','true');
      Object.assign(el.style,{position:'fixed',left:`${r.x}px`,top:`${r.y}px`,width:`${r.width}px`,height:`${r.height}px`,margin:'0',pointerEvents:'none',zIndex:'10'});
      document.body.append(el);
      el.animate([{opacity:1,transform:'none'},{opacity:0,transform:reduced.matches?'none':'translateY(-4px)'}],{duration:120,easing:'cubic-bezier(.23,1,.32,1)'}).finished.then(()=>el.remove(),()=>el.remove());
    });
    records=next;
  };
  const observer=new MutationObserver(changes=> {
    if (changes.some(c=>[...c.addedNodes,...c.removedNodes].some(n=>n.nodeType===1 && !n.hasAttribute('data-motion-ghost'))) && !frame) frame=requestAnimationFrame(sample);
  });
  observer.observe(document.querySelector('#main') || document.body,{childList:true,subtree:true});
  sample();
  // Browser snapshots retain the outgoing route. Unsupported browsers navigate immediately.
  const navigate = event => {
    const link=event.target.closest?.('a[href]');
    if (replay || keyboard || !document.startViewTransition || event.button!==0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey || !link || link.target || link.hasAttribute('download') || link.relList.contains('external')) return;
    const url=new URL(link.href);
    if (url.origin!==location.origin || url.pathname===location.pathname || !url.pathname.startsWith('/')) return;
    event.preventDefault(); event.stopImmediatePropagation();
    const transition = document.startViewTransition(async()=> {
      replay=true;
      try { link.click(); } finally { replay=false; }
      // Rendering is suspended during a view-transition update. A frame wait can
      // deadlock here; let Leptos flush its microtasks with one ordinary task.
      await new Promise(resolve=>setTimeout(resolve,0));
    });
    // Hard navigations, rapid replacement and hidden tabs can skip snapshots.
    // The navigation itself still completes normally.
    transition.ready.catch(()=>{});
    transition.finished.catch(()=>{});
    transition.updateCallbackDone.catch(()=>{});
  };
  document.addEventListener('click',navigate,true);
  document.addEventListener('keydown',keydown,true);
  document.addEventListener('pointerdown',pointerdown,true);
  return () => { observer.disconnect(); cancelAnimationFrame(frame); document.removeEventListener('click',navigate,true); document.removeEventListener('keydown',keydown,true); document.removeEventListener('pointerdown',pointerdown,true); };
}
