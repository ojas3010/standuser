// Runs before the first paint: an opened share link (#c=...) is not the sample card, so style.css leaves the field neutral for it until
// app.js has drawn the link's own card.
if (location.hash.startsWith('#c=')) document.documentElement.classList.add('link');
