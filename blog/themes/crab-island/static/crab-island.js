function initMobile() {
  var icon = document.getElementById("mobile-navbar-icon");
  var menu = document.getElementById("mobile-menu");

  if (!icon || !menu) return;

  icon.addEventListener("click", function () {
    var isOpen = menu.classList.toggle("is-open");
    if (isOpen) {
      icon.classList.add("icon-click");
      icon.classList.remove("icon-out");
    } else {
      icon.classList.add("icon-out");
      icon.classList.remove("icon-click");
    }
  });
}

function initToc() {
  var tocLinks = document.querySelectorAll(".toc-link");
  var headerLinks = document.querySelectorAll(".post-content h1, .post-content h2");
  var tocLinkLis = document.querySelectorAll(".post-toc-content li");

  if (!tocLinks.length || !headerLinks.length) return;

  function findActiveIndex(headers, scrollTop) {
    scrollTop += 30;
    for (var i = 0; i < headers.length - 1; i++) {
      if (scrollTop > headers[i].offsetTop && scrollTop <= headers[i + 1].offsetTop) return i;
    }
    if (scrollTop > headers[headers.length - 1].offsetTop) return headers.length - 1;
    return -1;
  }

  document.addEventListener("scroll", function () {
    var scrollTop = document.body.scrollTop || document.documentElement.scrollTop;
    var activeIndex = findActiveIndex(headerLinks, scrollTop);

    tocLinks.forEach(function (el) { el.classList.remove("active"); });
    tocLinkLis.forEach(function (el) { el.classList.remove("has-active"); });

    if (activeIndex !== -1) {
      tocLinks[activeIndex].classList.add("active");
      var ancestor = tocLinks[activeIndex].parentNode;
      while (ancestor.tagName !== "NAV") {
        ancestor.classList.add("has-active");
        ancestor = ancestor.parentNode.parentNode;
      }
    }
  });
}

function initThemeToggle() {
  var buttons = document.querySelectorAll("#theme-toggle, #mobile-theme-toggle");
  if (!buttons.length) return;

  function currentTheme() {
    return document.documentElement.getAttribute("data-theme") ||
      (window.matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark");
  }

  function applyTheme(theme) {
    document.documentElement.setAttribute("data-theme", theme);
    localStorage.setItem("theme", theme);
    // In dark mode show sun (to switch to light), in light mode show moon (to switch to dark)
    document.querySelectorAll(".theme-icon-light").forEach(function (el) {
      el.style.display = theme === "dark" ? "inline" : "none";
    });
    document.querySelectorAll(".theme-icon-dark").forEach(function (el) {
      el.style.display = theme === "dark" ? "none" : "inline";
    });
  }

  // Apply initial state
  applyTheme(currentTheme());

  buttons.forEach(function (btn) {
    btn.addEventListener("click", function (e) {
      e.preventDefault();
      e.stopPropagation();
      applyTheme(currentTheme() === "dark" ? "light" : "dark");
    });
  });
}

function initKeyboardNav() {
  document.addEventListener("keydown", function (e) {
    if (e.target.tagName === "INPUT" || e.target.tagName === "TEXTAREA" ||
        e.target.tagName === "SELECT" || e.target.isContentEditable) return;
    var link;
    if (e.key === "ArrowLeft") {
      link = document.querySelector("a.nav-arrow.prev");
    } else if (e.key === "ArrowRight") {
      link = document.querySelector("a.nav-arrow.next");
    }
    if (link) link.click();
  });
}

if (document.readyState === "complete" ||
    (document.readyState !== "loading" && !document.documentElement.doScroll)) {
  initMobile();
  initToc();
  initKeyboardNav();
  initThemeToggle();
} else {
  document.addEventListener("DOMContentLoaded", function () {
    initMobile();
    initToc();
    initKeyboardNav();
    initThemeToggle();
  });
}
