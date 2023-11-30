const input = document.getElementById("image");
const button = document.getElementById("upload-button");

function uploadImage() {
  const file = input.files[0];

  if (!file) {
    alert("Please select an image file.");
    return;
  }

  const formData = new FormData();
  formData.append("image", file);

  button.disabled = true;

  fetch("/api/upload", {
    method: "POST",
    body: formData,
  })
    .then((response) => {
      if (response.ok) {
        alert("Image upload successfully.");
      } else {
        if (response.status === 413) {
          alert("Failed to upload image size is limit at 10MB.");
        } else {
          alert("Failed to upload image.");
        }
      }
    })
    .catch((error) => {
      console.error(error);
    })
    .finally(() => {
      button.disabled = false;
    });
}
