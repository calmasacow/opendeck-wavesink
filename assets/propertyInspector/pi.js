// Shared property inspector logic for the WaveSink actions.
//
// Markup conventions:
//   data-setting="name"          binds an input/select/checkbox to settings.name
//   data-choices="inputs|mixes|profiles"
//                                fills a select from WaveSink's live state; options
//                                already in the markup stay first
//   data-type="number"           stores the value as a number
//   data-show="setting=a,b"      shown only while settings.setting is a or b
//   data-controller="Keypad"     shown only on that controller

let settings = {};
let controller = "Keypad";
let save = (_changed) => {};

function bound() {
	return document.querySelectorAll("[data-setting]");
}

function readField(el) {
	if (el.type === "checkbox") return el.checked;
	if (el.dataset.type === "number") return el.value === "" ? null : Number(el.value);
	return el.value;
}

function writeField(el, value) {
	if (el.type === "checkbox") el.checked = value ?? el.dataset.default === "true";
	else el.value = value ?? el.dataset.default ?? "";
	if (el.tagName === "SELECT" && value != null && el.value !== String(value)) {
		// Saved choice WaveSink doesn't have (any more): keep it visible.
		const option = new Option(`${value} (missing)`, value);
		el.add(option);
		el.value = value;
	}
	const display = document.querySelector(`[data-display="${el.dataset.setting}"]`);
	if (display) display.textContent = el.value;
}

function applyVisibility() {
	for (const el of document.querySelectorAll("[data-show],[data-controller]")) {
		let shown = true;
		if (el.dataset.controller) shown = el.dataset.controller.split(",").includes(controller);
		if (shown && el.dataset.show) {
			const [key, values] = el.dataset.show.split("=");
			shown = values.split(",").includes(String(settings[key] ?? document.querySelector(`[data-setting="${key}"]`)?.dataset.default ?? ""));
		}
		el.style.display = shown ? "" : "none";
	}
}

function fill(choices) {
	document.getElementById("offline").style.display = choices.connected ? "none" : "";
	if (!choices.connected) return;
	for (const select of document.querySelectorAll("select[data-choices]")) {
		const keep = [...select.querySelectorAll("option[data-fixed]")];
		select.innerHTML = "";
		for (const option of keep) select.add(option);
		for (const item of choices[select.dataset.choices] ?? []) {
			const id = typeof item === "string" ? item : item.id;
			const label = typeof item === "string" ? item : item.label + (item.kind === "hardware" ? " (hardware)" : "");
			select.add(new Option(label, id));
		}
		writeField(select, settings[select.dataset.setting]);
	}
}

function connectElgatoStreamDeckSocket(inPort, inPropertyInspectorUUID, inRegisterEvent, inInfo, inActionInfo) {
	const actionInfo = JSON.parse(inActionInfo);
	settings = actionInfo.payload.settings ?? {};
	controller = actionInfo.payload.controller ?? "Keypad";
	document.body.dataset.action = actionInfo.action;
	if (typeof onAction === "function") onAction(actionInfo.action);

	const websocket = new WebSocket("ws://127.0.0.1:" + inPort);
	const send = (event, payload) => websocket.send(JSON.stringify({ event, context: actionInfo.context, action: actionInfo.action, payload }));

	websocket.onopen = () => {
		websocket.send(JSON.stringify({ event: inRegisterEvent, uuid: inPropertyInspectorUUID }));
		send("sendToPlugin", { request: "choices" });
	};
	websocket.onmessage = (message) => {
		const data = JSON.parse(message.data);
		if (data.event === "sendToPropertyInspector" && data.payload?.event === "choices") fill(data.payload);
		if (data.event === "didReceiveSettings") {
			settings = data.payload.settings ?? {};
			for (const el of bound()) writeField(el, settings[el.dataset.setting]);
			applyVisibility();
		}
	};

	// Only the field that changed: two fields may share a setting (a key's
	// step and a dial's per-tick step), and the hidden one must not win.
	save = (changed) => {
		settings[changed.dataset.setting] = readField(changed);
		for (const el of bound()) writeField(el, settings[el.dataset.setting]);
		applyVisibility();
		send("setSettings", settings);
	};

	for (const el of bound()) {
		writeField(el, settings[el.dataset.setting]);
		el.addEventListener(el.type === "range" ? "input" : "change", () => save(el));
	}
	applyVisibility();
}
