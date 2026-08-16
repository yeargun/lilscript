import "./site.js";

if (new URLSearchParams(window.location.search).has("embed")) {
  document.body.classList.add("embed");
}

const listings = [
  { id: 0, name: "Field Notes Set", category: "Paper goods", seller: "Mori Studio", badge: "Small batch", art: "📓", tone: "clay", price: 1800 },
  { id: 1, name: "Pour-over No. 2", category: "Kitchen", seller: "North Foundry", badge: "Bestseller", art: "☕", tone: "ink", price: 4200 },
  { id: 2, name: "Linen Market Bag", category: "Everyday", seller: "Kindred Cloth", badge: "Low stock", art: "🧺", tone: "sage", price: 3200 },
  { id: 3, name: "Sunday Incense", category: "Home ritual", seller: "Quiet Hours", badge: "New", art: "🌿", tone: "sun", price: 1600 },
  { id: 4, name: "Studio Candle", category: "Home", seller: "Wax & Wane", badge: "Hand poured", art: "🕯️", tone: "rose", price: 2800 },
  { id: 5, name: "Pocket Radio", category: "Objects", seller: "Common Signal", badge: "Restored", art: "📻", tone: "blue", price: 6800 },
];

const root = document.querySelector("#parcel-market");
const status = document.querySelector("#market-status");
const quantities = listings.map(() => 0);
let screen = "market";

function money(cents) {
  return new Intl.NumberFormat("en-US", { style: "currency", currency: "USD" }).format(cents / 100);
}

function itemCount() {
  return quantities.reduce((total, quantity) => total + quantity, 0);
}

function subtotal() {
  return listings.reduce((total, listing) => total + listing.price * quantities[listing.id], 0);
}

function announce(message) {
  status.textContent = "";
  window.setTimeout(() => { status.textContent = message; }, 20);
}

function header(homeAction = "home") {
  const count = itemCount();
  return `<header class="market-topbar"><button class="market-brand" type="button" data-action="${homeAction}" aria-label="Parcel Market home"><span aria-hidden="true">P</span>parcel market</button><p>Independent objects for daily life</p><button class="market-list-pill" type="button" data-action="open-list" aria-label="Shopping list, ${count} ${count === 1 ? "item" : "items"}"><span>Shopping list</span><b aria-hidden="true">${count}</b></button></header>`;
}

function listingCard(listing) {
  const quantity = quantities[listing.id];
  const label = quantity > 0 ? `Add another ${listing.name}, ${quantity} already in list` : `Add ${listing.name} to shopping list`;
  return `<article class="market-listing-card" aria-labelledby="listing-${listing.id}"><div class="market-listing-art tone-${listing.tone}" aria-hidden="true"><span>${listing.art}</span><small>${listing.category}</small></div><div class="market-listing-copy"><div class="market-listing-meta"><span>${listing.badge}</span><span>${listing.seller}</span></div><h3 id="listing-${listing.id}">${listing.name}</h3><div class="market-listing-buy"><strong>${money(listing.price)}</strong><button class="market-add-button" type="button" data-action="add" data-id="${listing.id}" aria-label="${label}">${quantity > 0 ? "Add another" : "Add to list"}</button></div></div></article>`;
}

function cartRows() {
  return listings.filter((listing) => quantities[listing.id] > 0).map((listing) => `<div class="market-shopping-row"><span class="market-row-art tone-${listing.tone}" aria-hidden="true">${listing.art}</span><div><strong>${listing.name}</strong><small>${quantities[listing.id]} × ${money(listing.price)}</small></div><button type="button" data-action="remove" data-id="${listing.id}" aria-label="Remove one ${listing.name} from shopping list">−</button></div>`).join("");
}

function cart() {
  const count = itemCount();
  const body = count === 0
    ? `<div class="market-empty-list"><span aria-hidden="true">↗</span><h3>Your list is open</h3><p>Collect a few good things. They will wait here while you browse.</p></div>`
    : `<div class="market-list-rows">${cartRows()}</div><div class="market-list-total"><span>Subtotal</span><strong>${money(subtotal())}</strong></div><button class="market-checkout-button" type="button" data-action="checkout">Continue to fake payment</button><button class="market-clear-button" type="button" data-action="clear">Clear shopping list</button>`;
  return `<aside id="shopping-list" class="market-shopping-list" tabindex="-1" aria-labelledby="shopping-list-title"><div class="market-list-heading"><div><span class="market-eyebrow">Your edit</span><h2 id="shopping-list-title">Shopping list</h2></div><b aria-label="${count} ${count === 1 ? "item" : "items"}">${count}</b></div>${body}</aside>`;
}

function marketView() {
  return `<div class="market-shell">${header()}<div class="market-content"><section class="market-hero" aria-labelledby="market-title"><p class="market-eyebrow">Edition 04 · made by small studios</p><div class="market-hero-grid"><h2 id="market-title" tabindex="-1">Useful things,<br><em>chosen slowly.</em></h2><div class="market-hero-note"><p>A compact market for objects with a point of view. Six finds, no endless aisle.</p><span>Fresh shelf ↓</span></div></div></section><section class="market-layout" aria-labelledby="shelf-title"><div><div class="market-section-heading"><div><span class="market-eyebrow">This week</span><h2 id="shelf-title">The market shelf</h2></div><p>${listings.length} objects</p></div><div class="market-listing-grid">${listings.map(listingCard).join("")}</div></div>${cart()}</section></div><footer class="market-footer"><span>Parcel Market · Lastro POC</span><span>Small web, useful things.</span></footer></div>`;
}

function orderRows() {
  return listings.filter((listing) => quantities[listing.id] > 0).map((listing) => `<div class="market-order-row"><span>${listing.name} <small>× ${quantities[listing.id]}</small></span><strong>${money(listing.price * quantities[listing.id])}</strong></div>`).join("");
}

function checkoutView() {
  const shipping = 600;
  const total = subtotal() + shipping;
  return `<div class="market-shell market-checkout-shell">${header("back")}<div class="market-checkout-page"><button class="market-back-link" type="button" data-action="back">← Back to market</button><div class="market-checkout-intro"><span class="market-eyebrow">One last step</span><h2 id="checkout-title" tabindex="-1">Payment,<br><em>without the pressure.</em></h2><p>This is a demo checkout. Use any made-up details—nothing is sent or stored.</p></div><div class="market-payment-layout"><form class="market-payment-card" data-payment-form><div class="market-payment-title"><span aria-hidden="true">01</span><div><p class="market-eyebrow">Fake details</p><h3>Pay securely-ish</h3></div></div><div class="market-field"><label for="demo-email">Email</label><input id="demo-email" name="email" type="email" value="buyer@example.test" autocomplete="off" required></div><div class="market-field"><label for="demo-card">Fake card number</label><input id="demo-card" name="card" value="4242 4242 4242 4242" inputmode="numeric" autocomplete="off" required></div><div class="market-field-row"><div class="market-field"><label for="demo-expiry">Expiry</label><input id="demo-expiry" name="expiry" value="12 / 30" autocomplete="off" required></div><div class="market-field"><label for="demo-cvc">CVC</label><input id="demo-cvc" name="cvc" value="123" inputmode="numeric" autocomplete="off" required></div></div><button class="market-pay-button" type="submit">Complete fake payment <span>${money(total)} →</span></button><p class="market-demo-note">Demo only · no network request · no data saved</p></form><aside class="market-order-card" aria-labelledby="order-title"><div><span class="market-eyebrow">Order note</span><h3 id="order-title">Your parcel</h3></div><div class="market-order-rows">${orderRows()}</div><div class="market-order-row muted"><span>Flat shipping</span><strong>${money(shipping)}</strong></div><div class="market-grand-total"><span>Total</span><strong>${money(total)}</strong></div></aside></div></div></div>`;
}

function successView() {
  return `<div class="market-shell market-success-shell">${header()}<div class="market-success-card"><span class="market-success-mark" aria-hidden="true">✓</span><p class="market-eyebrow">Demo payment complete</p><h2 id="success-title" tabindex="-1">Your imaginary<br><em>parcel is packed.</em></h2><p>No card was charged and no order was placed. The entire flow ran locally in your browser.</p><button class="market-checkout-button" type="button" data-action="home">Return to the market</button></div></div>`;
}

function render({ focus } = {}) {
  root.innerHTML = screen === "checkout" ? checkoutView() : screen === "success" ? successView() : marketView();
  if (focus) window.requestAnimationFrame(() => root.querySelector(focus)?.focus());
}

function returnFocus(action, id) {
  if (action === "add") return `[data-action="add"][data-id="${id}"]`;
  if (action === "remove" && quantities[id] > 0) return `[data-action="remove"][data-id="${id}"]`;
  return "#shopping-list";
}

root.addEventListener("click", (event) => {
  const target = event.target instanceof Element ? event.target.closest("[data-action]") : null;
  if (!target || !root.contains(target)) return;
  const action = target.dataset.action;
  const id = Number.parseInt(target.dataset.id ?? "-1", 10);
  if (action === "add" && listings[id]) {
    quantities[id] += 1;
    announce(`${listings[id].name} added. ${itemCount()} ${itemCount() === 1 ? "item" : "items"} in the shopping list.`);
    render({ focus: returnFocus(action, id) });
  } else if (action === "remove" && listings[id] && quantities[id] > 0) {
    quantities[id] -= 1;
    announce(`One ${listings[id].name} removed. ${itemCount()} ${itemCount() === 1 ? "item remains" : "items remain"}.`);
    render({ focus: returnFocus(action, id) });
  } else if (action === "clear") {
    quantities.fill(0);
    announce("Shopping list cleared.");
    render({ focus: "#shopping-list" });
  } else if (action === "checkout" && itemCount() > 0) {
    screen = "checkout";
    render({ focus: "#checkout-title" });
    announce("Fake payment screen opened.");
    window.scrollTo({ top: 0, behavior: "smooth" });
  } else if (action === "open-list") {
    if (screen !== "market") { screen = "market"; render(); }
    root.querySelector("#shopping-list")?.scrollIntoView({ block: "start", behavior: "smooth" });
    root.querySelector("#shopping-list")?.focus();
  } else if (action === "back" || action === "home") {
    screen = "market";
    render({ focus: "#market-title" });
    announce("Marketplace opened.");
    window.scrollTo({ top: 0, behavior: "smooth" });
  }
});

root.addEventListener("submit", (event) => {
  if (!(event.target instanceof HTMLFormElement) || !event.target.matches("[data-payment-form]")) return;
  event.preventDefault();
  screen = "success";
  quantities.fill(0);
  render({ focus: "#success-title" });
  announce("Demo payment complete. No order was placed and no card was charged.");
  window.scrollTo({ top: 0, behavior: "smooth" });
});

window.setTimeout(() => {
  render();
  announce("Six marketplace listings loaded.");
}, 650);
