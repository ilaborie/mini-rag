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

if (document.readyState === "complete" ||
    (document.readyState !== "loading" && !document.documentElement.doScroll)) {
  initMobile();
  initToc();
} else {
  document.addEventListener("DOMContentLoaded", function () {
    initMobile();
    initToc();
  });
}
