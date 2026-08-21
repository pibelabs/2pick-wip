var track = document.getElementById("scroll-track");
var progress = document.getElementById("progress");
var titleLayer = document.getElementById("titleLayer");
var contentLayer = document.getElementById("contentLayer");
var notifyLayer = document.getElementById("notifyLayer");
var blueprintSvg = document.getElementById("blueprintSvg");
var factsBlock = document.getElementById("factsBlock");
var brandMark = document.getElementById("brandMark");
var notifyForm = document.getElementById("notifyForm");

var blueprintDrawn = false;
var factsShown = false;

function lerp(a, b, t) {
  return a + (b - a) * Math.max(0, Math.min(1, t));
}

function onScroll() {
  var scrollTop = track.scrollTop;
  var maxScroll = track.scrollHeight - window.innerHeight;
  var p = scrollTop / maxScroll; // 0..1

  progress.style.width = p * 100 + "%";

  /*
Phases:
0.00 – 0.20  WIP title visible, then scales up and fades
0.15 – 0.25  transition zone
0.25 – 0.70  blueprint + facts
0.65 – 0.80  transition zone
0.80 – 1.00  notify
*/

  // ── Title layer ──
  if (p < 0.22) {
    var titleProgress = p / 0.22;
    var scale = lerp(1, 2.5, titleProgress);
    var opacity = p < 0.12 ? 1 : lerp(1, 0, (p - 0.12) / 0.1);
    titleLayer.style.opacity = opacity;
    titleLayer.style.transform = "scale(" + scale + ")";
  } else {
    titleLayer.style.opacity = 0;
    titleLayer.style.transform = "scale(2.5)";
  }

  // ── Content layer ──
  if (p >= 0.18 && p < 0.75) {
    var fadeIn = lerp(0, 1, (p - 0.18) / 0.08);
    var fadeOut = p > 0.65 ? lerp(1, 0, (p - 0.65) / 0.1) : 1;
    contentLayer.style.opacity = Math.min(fadeIn, fadeOut);

    // trigger draw
    if (p > 0.22 && !blueprintDrawn) {
      blueprintSvg.classList.add("drawing");
      blueprintDrawn = true;
    }
    // trigger facts stagger
    if (p > 0.3 && !factsShown) {
      factsBlock.classList.add("facts-visible");
      factsShown = true;
    }
  } else {
    contentLayer.style.opacity = 0;
  }

  // ── Notify layer ──
  if (p >= 0.72) {
    var notifyFade = lerp(0, 1, (p - 0.72) / 0.1);
    notifyLayer.style.opacity = notifyFade;
    notifyLayer.classList.toggle("active", notifyFade > 0.5);
    track.style.pointerEvents = notifyFade > 0.5 ? "none" : "";
  } else {
    notifyLayer.style.opacity = 0;
    notifyLayer.classList.remove("active");
    track.style.pointerEvents = "";
  }

  // Brand mark
  brandMark.style.color = p > 0.2 && p < 0.75 ? "#444" : "#333";
}

track.addEventListener("scroll", onScroll, { passive: true });
onScroll();

// Block wheel from affecting <html>, force it into #scroll-track
document.addEventListener(
  "wheel",
  function (e) {
    e.preventDefault();
    track.scrollTop += e.deltaY;
  },
  { passive: false },
);

// Form
notifyForm.addEventListener("submit", function (e) {
  e.preventDefault();
  var input = notifyForm.querySelector("input");
  var btn = notifyForm.querySelector("button");
  var email = input.value.trim();

  if (!email.includes("@")) return;

  btn.disabled = true;
  btn.textContent = "...";

  fetch("/api/waitlist", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ email: email }),
  })
    .then(function (res) {
      return res.text().then(function (text) {
        return { ok: res.ok, text: text };
      });
    })
    .then(function (result) {
      btn.textContent = result.text;
      btn.style.background = "#333";
      input.disabled = true;
    })
    .catch(function () {
      btn.textContent = "Fehler — nochmal versuchen";
      btn.disabled = false;
    });
});
