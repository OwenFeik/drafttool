type Card = {
    name: string,
    image: string,
    set: string,
    rarity: "Mythic" | "Rare" | "Uncommon" | "Common" | "Special" | "Bonus",
    text: string
};

type PlayerList = [string, string][];

type ServerMessage =
    { type: "Started" }
    | { type: "Ended" }
    | { type: "FatalError", value: string }
    | { type: "Pack", "value": Card[] }
    | { type: "Passed" }
    | { type: "Finished", value: Card[] }
    | {
        type: "Connected",
        value: {
            draft: string,
            seat: string,
            players: PlayerList,
        }
    }
    | {
        type: "Reconnected",
        value: {
            draft: string,
            seat: string,
            in_progress: boolean,
            pool: Card[],
            pack?: Card[],
        }
    } | { type: "Refresh" };

type ClientMessage =
    { type: "HeartBeat" }
    | { type: "ReadyState", value: boolean }
    | { type: "Disconnected" }
    | { type: "SetName", value: string }
    | { type: "Pick", value: number };

enum Phase {
    Connecting,
    Lobby,
    Draft,
    Finished,
    Terminated,
}

type UiState =
    { phase: Phase.Connecting }
    | {
        phase: Phase.Lobby,
        updatePlayerList: (players: PlayerList) => void,
    }
    | {
        phase: Phase.Draft,
        receivePack: (pack: Card[]) => void,
        passedPack: () => void,
        updatePlayerList: (players: PlayerList) => void,
        updatePool: (pool: Card[]) => void,
    }
    | {
        phase: Phase.Finished,
        updatePlayerList: (players: PlayerList) => void,
        updatePool: (pool: Card[]) => void,
    }
    | {
        phase: Phase.Terminated,
        displayErrorMessage: (message: string) => void,
    };

type State = {
    draft: string | null,
    seat: string | null,
    ui: UiState,
    socket: WebSocket | null,
    reconnectAttempts: number,
};

let state: State = {
    draft: null,
    seat: null,
    ui: { phase: Phase.Connecting },
    socket: null,
    reconnectAttempts: 0,
};

/**
 * Send a message throug the websocket to the server, if the websocket is open.
 * @param message Message to send on the socket to the server.
 * @returns true if the socket was open and the message sent, else false.
 */
function sendMessage(message: ClientMessage): boolean {
    if (state.socket == null) {
        console.warn("Not sending message as socket not available.", message);
        return false;
    }

    state.socket.send(JSON.stringify(message));
    return true;
}

function phaseRootElement(phase: Phase): HTMLElement {
    switch (phase) {
        case Phase.Connecting:
            return document.getElementById("connecting")!;
        case Phase.Lobby:
            return document.getElementById("lobby")!;
        case Phase.Draft:
            return document.getElementById("draft")!;
        case Phase.Finished:
            return document.getElementById("finished")!;
        case Phase.Terminated:
            return document.getElementById("terminated")!;
    }
}

function setVisible(el: HTMLElement, visible: boolean) {
    if (visible) {
        el.style.display = "unset";
    } else {
        el.style.display = "none";
    }
}

function el(tag: string, parent?: HTMLElement): HTMLElement {
    let element = document.createElement(tag);
    if (parent) {
        parent.appendChild(element);
    }
    return element;
}

function checkbox(parent?: HTMLElement): HTMLInputElement {
    let element = attr(el("input", parent), "type", "checkbox");
    return element as HTMLInputElement;
}

function classes<E extends HTMLElement>(element: E, ...classes: string[]): E {
    element.classList.add(...classes);
    return element;
}

function text(element: HTMLElement, text: string): HTMLElement {
    element.innerText = text;
    return element;
}

function attr(element: HTMLElement, key: string, value?: string): HTMLElement {
    if (value !== undefined) {
        element.setAttribute(key, value);
    } else {
        element.toggleAttribute(key, true);
    }
    return element;
}

function forEachEl(selector: string, callback: (e: HTMLElement) => void) {
    document.querySelectorAll(selector)
        .forEach(el => callback(el as HTMLElement));
}

function setUpLobby(root: HTMLElement): UiState {
    let float = classes(el("div", root), "floating-centered", "simple-border");
    let table = classes(el("table", float), "padded");
    let headrow = el("tr", el("thead", table));
    text(el("td", headrow), "Status");
    text(el("td", headrow), "User");
    text(el("td", headrow), "Ready");

    let playerList = el("tbody", table);
    const updatePlayerList = (players: PlayerList) => {
        playerList.innerHTML = "";
        players.forEach(player => {
            let [id, name] = player;
            name = name != "" ? name : "No name";

            let row = attr(el("tr", playerList), "data-player", id);
            classes(text(el("td", row), "status"), "player-status");
            classes(text(el("td", row), name), "player-name");
            let ready = classes(checkbox(el("td", row)), "player-ready");
            
            if (id == state.seat) {
                ready.oninput = () => sendMessage(
                    { type: "ReadyState", value: ready.checked }
                );
            } else {
                attr(ready, "disabled");
            }
        });
    };

    return {
        phase: Phase.Lobby,
        updatePlayerList
    };
}

function populatePack(root: HTMLElement, cards: Card[]) {
    root.innerHTML = "";
    if (cards.length == 0) {
        text(classes(el("div", root), "floating-centered"), "Empty pack.");
    }

    let index = 0;
    cards.forEach(card => {
        let img = el("img", root);
        attr(img, "src", card.image);
        attr(img, "data-index", String(index));
        classes(img, "pack-card");

        img.onclick = () => {
            if (img.classList.contains("selected")) {
                if (img.dataset.index === undefined) {
                    console.error("Card with no index. Can't pick.", img);
                    return;
                }
                sendMessage({
                    type: "Pick",
                    value: parseInt(img.dataset.index)
                });
            } else {
                forEachEl(
                    ".pack-card.selected",
                    card => card.classList.remove("selected")
                );
                img.classList.add("selected");
            }
        };

        index++;
    });
}

function setUpDraft(root: HTMLElement): UiState {
    let float = el("div", root);
    let header = classes(el("div", float), "container");
    let pack = classes(el("div", float), "container");
    let pool = classes(el("div", float), "container");

    // TODO implement header with player list, other info
    text(classes(el("div", header), "floating-centered"), "Header");
    let widthBox = el("span", header);
    text(el("span", widthBox), "Card size");
    let width = el("input", widthBox) as HTMLInputElement;
    attr(width, "type", "range")
    attr(width, "min", "40");
    attr(width, "max", "400");
    attr(width, "value", "128");

    const updateCardWidths = () => {
        let w = width.value + "px";
        forEachEl(".pack-card", card => card.style.width = w);
    };

    width.oninput = () => updateCardWidths();

    // TODO implement pool element with picked cards
    text(classes(el("div", pool), "floating-centered"), "Picked cards.");

    const receivePack = (cards: Card[]) => {
        populatePack(pack, cards);
        updateCardWidths();
    };

    const passedPack = () => {
        pack.innerHTML = "";
        text(
            classes(el("div", pack), "floating-centered"),
            "Waiting for pack."
        );
    };

    return {
        phase: Phase.Draft,
        receivePack,
        passedPack,
        updatePlayerList: null!, // TODO
        updatePool: null!,       // TODO
    };
}

function setUpFinished(root: HTMLElement): UiState {
    // TODO set up post-draft pool view. Deck builder?
    return null!;
}

function setUpTerminated(root: HTMLElement): UiState {
    // TODO set up the page to display a fatal error.
    return null!;
}

function moveToPhase(phase: Phase) {
    if (phase == state.ui.phase) {
        // Already in this phase; no need to change anything.
        return;
    }

    setVisible(phaseRootElement(state.ui.phase), false);

    let root = phaseRootElement(phase);
    root.innerHTML = ""; // Reset the phase UI if we've rendered it.
    setVisible(root, true);
    switch (phase) {
        case Phase.Lobby:
            state.ui = setUpLobby(root);
            break;
        case Phase.Draft:
            state.ui = setUpDraft(root);
            break;
        case Phase.Finished:
            state.ui = setUpFinished(root);
            break;
        case Phase.Terminated:
            state.ui = setUpTerminated(root);
            break;
    }
}

function displayErrorMessage(message: string) {
    console.error(message);
    if (state.ui.phase == Phase.Terminated) {
        state.ui.displayErrorMessage(message);
    } else {
        console.warn("Call to displayErrorMessage when phase not Terminated.");
    }
}

function terminate(message: string) {
    moveToPhase(Phase.Terminated);
    displayErrorMessage(message);
}

function receivedPack(pack: Card[]) {
    if (state.ui.phase == Phase.Draft) {
        state.ui.receivePack(pack);
    } else {
        console.warn("Can't display pack in phase", state.ui.phase);
    }
}

function passedPack() {
    if (state.ui.phase == Phase.Draft) {
        state.ui.passedPack();
    } else {
        console.warn("Can't pass pack in phase", state.ui.phase);
    }
}

function updatePool(pool: Card[]) {
    if (state.ui.phase == Phase.Draft || state.ui.phase == Phase.Finished) {
        state.ui.updatePool(pool);
    } else {
        console.warn("Can't update pool in phase", state.ui.phase);
    }
}

function updatePlayerList(playerList: PlayerList) {
    switch (state.ui.phase) {
        case Phase.Lobby:
        case Phase.Draft:
        case Phase.Finished:
            state.ui.updatePlayerList(playerList);
            break;
        default:
            console.warn("Can't update player list in phase", state.ui.phase);
            break;
    }
}

function updateDraftSeat(draft: string, seat: string) {
    state.draft = draft;
    state.seat = seat;
    seatToLocalStorage(draft, seat);
}

function handleMessage(message: ServerMessage) {
    switch (message.type) {
        case "Started":
            terminate("Failed to join draft. Draft has already started.");
            break;
        case "Ended":
            moveToPhase(Phase.Finished);
            break;
        case "FatalError":
            terminate("Server error: " + message.value);
            break;
        case "Pack":
            moveToPhase(Phase.Draft);
            receivedPack(message.value);
            break;
        case "Passed":
            passedPack();
            break;
        case "Finished":
            moveToPhase(Phase.Finished);
            updatePool(message.value);
            break;
        case "Connected":
            moveToPhase(Phase.Lobby);
            updateDraftSeat(message.value.draft, message.value.seat);
            updatePlayerList(message.value.players);
            break;
        case "Reconnected":
            let draft_in_progress = message.value.in_progress;
            moveToPhase(draft_in_progress ? Phase.Draft : Phase.Finished);
            updateDraftSeat(message.value.draft, message.value.seat);
            updatePool(message.value.pool);
            if (message.value.pack) {
                receivedPack(message.value.pack);
            }
            break;
        case "Refresh":
            location.href = location.href;
            break;
    }
}

function determineDraftId(): string | null {
    const url = new URL(location.href);
    const path = url.pathname;
    const parts = path.split("/");

    if (parts.length == 0) {
        return null;
    }

    const uuid = parts[parts.length - 1];

    if (!/^[0-9a-f\-]{36}$/.test(uuid)) {
        return null;
    }

    return uuid;
}

function seatFromLocalStorage(draftId: string): string | null {
    return localStorage.getItem(draftId);
}

function seatToLocalStorage(draftId: string, seatId: string) {
    localStorage.setItem(draftId, seatId);
}

function openWebsocket(draftId: string) {
    const MAX_RECONNECT_ATTEMPTS = 10;

    let protocol = location.protocol == "https" ? "wss" : "ws";
    let url = `${protocol}://${location.host}/ws/${draftId}`;
    let seatId = seatFromLocalStorage(draftId);
    if (seatId != null) {
        url = url + "/" + seatId;
    }

    const ws = new WebSocket(url);
    ws.onopen = e => {
        console.log("Websocket opened.");
        state.reconnectAttempts = 0;
        state.socket = ws;
    };
    ws.onmessage = e => {
        e.data.text().then((json: string) => handleMessage(JSON.parse(json)));
    };
    ws.onclose = e => { state.socket = null; };
    ws.onerror = e => {
        console.error("Websocket error:", e);
        if (state.reconnectAttempts < MAX_RECONNECT_ATTEMPTS) {
            state.reconnectAttempts++;
            openWebsocket(draftId);
        } else {
            console.log("Maximum number of reconnect attempts exceeded.");
            displayErrorMessage("Connection error.");
        }
    };
}

function main() {
    const draftId = determineDraftId();
    if (draftId == null) {
        terminate("Draft ID not found in URL.");
        return;
    }

    openWebsocket(draftId);
}

window.onload = main
