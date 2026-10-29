/* app.js — page behaviour: scroll reveals and the nav's scrolled state. */
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

  const year = document.querySelector('[data-year]');
  if (year) year.textContent = new Date().getFullYear();
})();
