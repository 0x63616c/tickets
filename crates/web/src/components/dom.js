// Small DOM bridge: top-layer positioning and progressive motion. No UI state lives here.
export function position_popup(trigger, panel) {
  if (!trigger?.isConnected || !panel?.isConnected) return;
  if (panel.showPopover) {
    try { if (!panel.matches(':popover-open')) panel.showPopover(); } catch (_) { /* older engine */ }
  }
  const r = trigger.getBoundingClientRect();
  const gap = 4, edge = 8;
  panel.style.maxWidth = `${innerWidth - edge * 2}px`;
  panel.style.minWidth = `${Math.min(r.width, innerWidth - edge * 2)}px`;
  const below = innerHeight - r.bottom - gap - edge, above = r.top - gap - edge;
  const up = below < Math.min(panel.scrollHeight, 280) && above > below;
  panel.dataset.side = up ? 'top' : 'bottom';
  panel.style.maxHeight = `${Math.max(48, Math.min(320, up ? above : below))}px`;
  const p = panel.getBoundingClientRect();
  const left = Math.max(edge, Math.min(r.left, innerWidth - p.width - edge));
  panel.style.left = `${left}px`;
  panel.style.top = `${up ? Math.max(edge, r.top - p.height - gap) : r.bottom + gap}px`;
  panel.style.transformOrigin = trigger.classList.contains('account-trigger') ? `left ${up ? 'bottom' : 'top'}` : `${Math.max(0, Math.min(p.width, r.left + r.width / 2 - left))}px ${up ? 'bottom' : 'top'}`;
}
export function hide_popup(panel) {
  try { if (panel.matches(':popover-open')) panel.hidePopover(); } catch (_) { /* fallback */ }
}
export function install_motion() {
  const reduced = { get matches() { return document.documentElement.dataset.motion === 'reduce' || matchMedia('(prefers-reduced-motion: reduce)').matches; } };
  const scheme = matchMedia('(prefers-color-scheme: light)');
  const schemeChanged = () => { try { if (localStorage.getItem('tickets.theme') === 'system') document.documentElement.dataset.theme = scheme.matches ? 'light' : 'dark'; } catch (_) {} };
  scheme.addEventListener('change', schemeChanged);
  let records = new Map(), frame = 0, keyboard = false, replay = false, route = location.pathname;
  const selector = '[data-motion-key]';
  const keydown = () => { keyboard = true; };
  const pointerdown = () => { keyboard = false; };
  const sample = () => {
    frame = 0;
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
  return () => { scheme.removeEventListener('change', schemeChanged); observer.disconnect(); cancelAnimationFrame(frame); document.removeEventListener('click',navigate,true); document.removeEventListener('keydown',keydown,true); document.removeEventListener('pointerdown',pointerdown,true); };
}

export function watch_popup(trigger, panel) {
  const update = () => { if (!panel.hidden && panel.dataset.state === 'open') position_popup(trigger,panel); };
  window.addEventListener('scroll',update,true);
  window.addEventListener('resize',update);
  return () => { window.removeEventListener('scroll',update,true); window.removeEventListener('resize',update); };
}
