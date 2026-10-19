"use strict";
(() => {
  const input = document.getElementById("handbook-search");
  const status = document.getElementById("search-status");
  const clear = document.getElementById("clear-search");
  const articles = [...document.querySelectorAll("article[data-document]")];
  const links = [...document.querySelectorAll("nav a[data-page]")];
  const groups = [...document.querySelectorAll(".nav-group")];
  const text = new Map(
    articles.map((article) => [
      article.id,
      article.textContent.toLocaleLowerCase(),
    ]),
  );
  const filter = () => {
    const terms = input.value
      .trim()
      .toLocaleLowerCase()
      .split(/\s+/)
      .filter(Boolean);
    let count = 0;
    for (const article of articles) {
      const match = terms.every((term) => text.get(article.id).includes(term));
      article.hidden = !match;
      if (match) count += 1;
    }
    for (const link of links) {
      link.parentElement.hidden = document.getElementById(
        link.dataset.page,
      ).hidden;
    }
    for (const group of groups) {
      group.hidden = [...group.querySelectorAll("li")].every(
        (item) => item.hidden,
      );
      if (terms.length) group.open = true;
    }
    status.textContent = terms.length
      ? `${count} of ${articles.length} pages match. Select a chapter to read it.`
      : "";
    clear.hidden = terms.length === 0;
  };
  const revealTarget = (hash = location.hash) => {
    let target;
    try {
      target = document.getElementById(decodeURIComponent(hash.slice(1)));
    } catch {
      return;
    }
    const article = target?.closest("article");
    if (article?.hidden) {
      input.value = "";
      filter();
    }
    for (const link of links) {
      if (link.dataset.page === article?.id)
        link.setAttribute("aria-current", "location");
      else link.removeAttribute("aria-current");
    }
    if (target && article) requestAnimationFrame(() => target.scrollIntoView());
  };
  document.addEventListener("click", (event) => {
    if (
      event.defaultPrevented ||
      event.button !== 0 ||
      event.ctrlKey ||
      event.metaKey ||
      event.altKey ||
      event.shiftKey
    )
      return;
    const anchor = event.target.closest("a[href^='#']");
    if (anchor) revealTarget(anchor.hash);
  });
  input.addEventListener("input", filter);
  clear.addEventListener("click", () => {
    input.value = "";
    filter();
    input.focus();
  });
  document.addEventListener("keydown", (event) => {
    if (
      event.key === "/" &&
      !event.ctrlKey &&
      !event.metaKey &&
      !event.altKey &&
      !["INPUT", "TEXTAREA", "SELECT"].includes(document.activeElement.tagName)
    ) {
      event.preventDefault();
      input.focus();
    }
    if (event.key === "Escape" && document.activeElement === input) {
      input.value = "";
      filter();
    }
  });
  document
    .getElementById("print-handbook")
    .addEventListener("click", (event) => {
      event.preventDefault();
      window.print();
    });
  window.addEventListener("hashchange", () => revealTarget());
  revealTarget();
})();
