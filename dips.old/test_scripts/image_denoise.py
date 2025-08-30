import numpy as np
import matplotlib.pyplot as plt
import cv2

def display_fft(image_path):
    # Load the image in grayscale
    img = cv2.imread(image_path, cv2.IMREAD_GRAYSCALE)
    
    if img is None:
        print("Error: Could not load image.")
        return

    # Compute the FFT
    f = np.fft.fft2(img)
    fshift = np.fft.fftshift(f)  # Shift zero frequency to center
    magnitude_spectrum = 20 * np.log(np.abs(fshift) + 1)  # Add 1 to avoid log(0)

    rows, cols = img.shape
    crow, ccol = rows // 2, cols // 2

    mask = np.zeros((rows, cols), np.uint8)
    r = 75
    mask[crow - r:crow + r, ccol - r:ccol + r] = 1

    fshift_masked = fshift * mask

    # fft reverse
    f_ishift = np.fft.ifftshift(fshift_masked)
    img_back = np.fft.ifft2(f_ishift)
    img_back = np.abs(img_back)
    
    # Plot original image and its FFT
    plt.figure(figsize=(16, 6))

    plt.subplot(1, 3, 1)
    plt.title('Original Image')
    plt.imshow(img, cmap='gray')
    plt.axis('off')

    plt.subplot(1, 3, 2)
    plt.title('FFT Magnitude Spectrum')
    plt.imshow(magnitude_spectrum, cmap='gray')
    plt.axis('off')

    
    plt.subplot(1, 3, 3)
    plt.title('Reconstructed FFt')
    plt.imshow(img_back, cmap='gray')
    plt.axis('off')

    plt.tight_layout()
    plt.show()

# Example usage
image_path = 'test_files/output.png'  # Replace with your image path
display_fft(image_path)
