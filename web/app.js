/* app.js — page behaviour: scroll reveals, the nav's scrolled state and the phone menu. */
(function () {
  'use strict';

  const root = document.documentElement;
  root.classList.add('js');

  const items = document.querySelectorAll('[data-reveal]');
  if ('IntersectionObserver' in window) {
    const io = new IntersectionObserver(
      (entries) => {
        for (const e of entries) {
          if (!e.isIntersecting) continue;
          e.target.classList.add('in');
          io.unobserve(e.target);
        }
      },
      { rootMargin: '0px 0px -8% 0px', threshold: 0.08 }
    );
    items.forEach((node, i) => {
      // stagger siblings that enter together
      node.style.transitionDelay = `${(i % 3) * 0.08}s`;
      io.observe(node);
    });
  } else {
    items.forEach((node) => node.classList.add('in'));
  }

  const nav = document.querySelector('.nav');
  const onScroll = () => nav.classList.toggle('scrolled', window.scrollY > 8);
  window.addEventListener('scroll', onScroll, { passive: true });
  onScroll();

  // phone menu: toggles the link list; closes on a link tap or Escape
  const toggle = nav.querySelector('.nav-toggle');
  if (toggle) {
    const setOpen = (open) => {
      nav.classList.toggle('open', open);
      toggle.setAttribute('aria-expanded', String(open));
      toggle.setAttribute('aria-label', open ? 'Close menu' : 'Open menu');
    };
    toggle.addEventListener('click', () => setOpen(!nav.classList.contains('open')));
    nav.querySelectorAll('.nav-links a').forEach((a) => a.addEventListener('click', () => setOpen(false)));
    document.addEventListener('keydown', (e) => {
      if (e.key === 'Escape' && nav.classList.contains('open')) {
        setOpen(false);
        toggle.focus();
      }
    });
  }

  // tables marked .stack collapse into labelled rows on phones; each cell
  // takes its column header (or the header's data-short) as data-label,
  // including rows added later
  const labelTables = () =>
    document.querySelectorAll('table.stack').forEach((t) => {
      const heads = [...t.querySelectorAll('thead th')].map((th) => th.dataset.short || th.textContent);
      t.querySelectorAll('tbody td:not([colspan]):not([data-label])').forEach((td) => {
        const head = heads[td.cellIndex];
        if (head) td.setAttribute('data-label', head);
      });
    });
  labelTables();
  new MutationObserver(labelTables).observe(document.body, { childList: true, subtree: true });

  const year = document.querySelector('[data-year]');
  if (year) year.textContent = new Date().getFullYear();
})();
