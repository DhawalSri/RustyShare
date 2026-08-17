
const IS_LOCAL_FILE = window.location.protocol === "file:";
const BASE_URL = IS_LOCAL_FILE ? "http://localhost:3000" : "";

// Dynamically check if we need a secure WebSocket
const wsProtocol = window.location.protocol === "https:" ? "wss:" : "ws:";
const WS_URL = IS_LOCAL_FILE ? "ws://localhost:3000/ws" : `${wsProtocol}//${window.location.host}/ws`;
let myDeviceId = null;
let pendingDownloadCode = null;
let ws;

// Initialize the WebSocket Connection
function connectWebSocket() {
    ws = new WebSocket(WS_URL);

    ws.onopen = () => console.log("Connected to Rust WebSocket");

    ws.onmessage = (event) => {
        const data = JSON.parse(event.data);

        // Server assigned us an ID
        if (data.type === "welcome") {
            myDeviceId = data.id;
            document.getElementById("myDeviceBadge").innerText = `My ID: ${myDeviceId}`;
        }
        
        // Server broadcasted the active device list
        else if (data.type === "device_list") {
            renderNearbyDevices(data.devices);
        }

        // Someone is trying to send US a file!
        else if (data.type === "transfer_request" && data.to === myDeviceId) {
            showTransferModal(data.from, data.fileName, data.code);
        }
    };

    ws.onclose = () => {
        console.log("Disconnected. Reconnecting in 3s...");
        document.getElementById("myDeviceBadge").innerText = "Disconnected.";
        setTimeout(connectWebSocket, 3000);
    };
}

// Start WebSocket immediately
connectWebSocket();

// Render the "Radar" Buttons
function renderNearbyDevices(devices) {
    const container = document.getElementById("nearbyDevices");
    container.innerHTML = ""; // Clear loading text

    // Filter out our own device
    const others = devices.filter(d => d !== myDeviceId);

    if (others.length === 0) {
        container.innerHTML = `<p class="text-muted">No nearby devices found.</p>`;
        return;
    }

    others.forEach(targetId => {
        const btn = document.createElement("button");
        btn.className = "device-btn";
        btn.innerText = `Send to ${targetId}`;
        btn.onclick = () => sendToDevice(targetId);
        container.appendChild(btn);
    });
}

// 3. The "AirDrop" Send Function
async function sendToDevice(targetId) {
    const fileInput = document.getElementById('fileInput');
    if (!fileInput.files[0]) {
        alert("Please select a file first!");
        return;
    }

    const file = fileInput.files[0];
    const formData = new FormData();
    formData.append("file", file);

    try {
        // Step 1: Upload the file silently to get a 6-digit code
        const response = await fetch(`${BASE_URL}/upload`, { method: 'POST', body: formData });
        if (!response.ok) throw new Error("Upload failed");
        const data = await response.json();

        // Step 2: Ping the target device over WebSocket with the code
        ws.send(JSON.stringify({
            type: "transfer_request",
            from: myDeviceId,
            to: targetId,
            fileName: file.name,
            code: data.code
        }));

        alert(`Request sent to ${targetId}! Waiting for them to accept.`);
    } catch (error) {
        console.error(error);
        alert("Failed to send to device.");
    }
}

// 4. Modal Logic (Receiving a file)
function showTransferModal(fromDevice, fileName, code) {
    pendingDownloadCode = code;
    document.getElementById("modalText").innerHTML = `<b>${fromDevice}</b> wants to send you:<br><br><i>${fileName}</i>`;
    document.getElementById("transferModal").style.display = "flex";
}

function acceptTransfer() {
    document.getElementById("transferModal").style.display = "none";
    if (pendingDownloadCode) {
        window.location.href = `${BASE_URL}/download/${pendingDownloadCode}`;
        pendingDownloadCode = null;
    }
}

function rejectTransfer() {
    document.getElementById("transferModal").style.display = "none";
    pendingDownloadCode = null;
}

// 5. Original HTTP Fallback Methods (Unchanged)
async function uploadFile() {
    const fileInput = document.getElementById('fileInput');
    const codeDisplay = document.getElementById('codeDisplay');
    if (!fileInput.files[0]) return alert("Please select a file first!");

    const formData = new FormData();
    formData.append("file", fileInput.files[0]);

    try {
        codeDisplay.innerText = "Uploading...";
        const response = await fetch(`${BASE_URL}/upload`, { method: 'POST', body: formData });
        if (!response.ok) throw new Error("Upload failed");
        const data = await response.json();
        codeDisplay.innerText = data.code; 
    } catch (error) {
        alert("Error uploading file.");
        codeDisplay.innerText = "---";
    }
}

function downloadFile() {
    const code = document.getElementById('codeInput').value;
    if (code.length !== 6) return alert("Please enter a valid 6-digit code.");
    window.location.href = `${BASE_URL}/download/${code}`;
}
