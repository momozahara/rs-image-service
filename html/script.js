const input = document.getElementById("images");
const button = document.getElementById("upload-button");

function uploadImage() {
  const files = input.files;

  if (files.length === 0) {
    alert("Please select an image file.");
    return;
  }

  const formData = new FormData();
  Array.from(files).forEach((file) => {
    formData.append("images", file);
  });

  button.disabled = true;

  fetch("/api/upload", {
    method: "POST",
    body: formData,
  })
    .then((response) => {
      if (response.ok) {
        alert("Image upload successfully.");
      } else {
        throw new Error();
      }
    })
    .catch(() => {
      alert("Failed to upload image.");
    })
    .finally(() => {
      button.disabled = false;
    });
}
