// const BASE_URL = 'http://10.251.222.166:3000';
const BASE_URL = '';

async function uploadFile() {
    const fileInput = document.getElementById('fileInput');
    const codeDisplay = document.getElementById('codeDisplay');
    
    if (!fileInput.files[0]) {
        alert("Please select a file first!");
        return;
    }

    const formData = new FormData();
    formData.append("file", fileInput.files[0]);

    try {
        codeDisplay.innerText = "Uploading...";
        const response = await fetch(`${BASE_URL}/upload`, {
            method: 'POST',
            body: formData
        });

        if (!response.ok) throw new Error("Upload failed");

        const data = await response.json();
        codeDisplay.innerText = data.code; 
    } catch (error) {
        console.error(error);
        alert("Error uploading file. Is the Rust server running?");
        codeDisplay.innerText = "";
    }
}

function downloadFile() {
    const code = document.getElementById('codeInput').value;
    if (code.length !== 6) {
        alert("Please enter a valid 6-digit code.");
        return;
    }
    // This triggers the browser download by redirecting to the Rust endpoint
    window.location.href = `${BASE_URL}/download/${code}`;
}