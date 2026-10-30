// Installed before navigation: observe every painted frame, including unstyled mounts.
window.loadingFrames = [];
window.loadingProbeDone = false;
const box = e => ({width:e.scrollWidth, height:e.scrollHeight, clientWidth:e.clientWidth, clientHeight:e.clientHeight});
function loadingFrame() {
    const d = document.documentElement, main = document.querySelector('main');
    if (d) {
        const frame = {time:performance.now(), document:box(d), main:main ? box(main) : null,
            phase:document.querySelector('[data-ar-t]') ? 'decode' : document.querySelector('.ars') ? 'skeleton' : 'content'};
        const previous = window.loadingFrames.at(-1);
        // Keep geometry offenders at changing extents; scanning every node every frame
        // would itself distort CPU-throttled animation timing.
        if (!previous || ['width','height'].some(k => frame.document[k] > previous.document[k]) ||
            (frame.main && previous.main && ['width','height'].some(k => frame.main[k] > previous.main[k]))) {
            frame.offenders = [...document.querySelectorAll('body *')].map(e => {
                const r=e.getBoundingClientRect(), s=getComputedStyle(e);
                return {element:e.tagName.toLowerCase(), class:e.getAttribute('class'),
                    x:r.x,y:r.y,right:r.right,bottom:r.bottom,width:r.width,height:r.height,overflow:s.overflow,position:s.position};
            }).filter(r => r.right>d.clientWidth+1 || r.bottom>d.clientHeight+1 || r.x < -1).slice(-40);
        }
        window.loadingFrames.push(frame);
    }
    if (!window.loadingProbeDone) requestAnimationFrame(loadingFrame);
}
requestAnimationFrame(loadingFrame);
