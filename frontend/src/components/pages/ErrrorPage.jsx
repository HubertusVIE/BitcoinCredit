import React, { useContext } from "react";
import closeIcon from "../../assests/close-btn.svg";
import logo from "../../assests/logo.png";
import { MainContext } from "../../context/MainContext";

export default function ErrrorPage() {
  const { handlePage } = useContext(MainContext);
  return (
    <div className="error">
      <div className="error-head">
        <span className="error-head-title">
          <img src={logo} />
        </span>
        <span className="close-btn" onClick={() => handlePage("home")}>
          <svg
            xmlns="http://www.w3.org/2000/svg"
            viewBox="0 0 16 16"
            fill="none"
          >
            <path
              fill-rule="evenodd"
              clip-rule="evenodd"
              d="M0.393822 0.393822C-0.131274 0.918918 -0.131274 1.77027 0.393822 2.29536L6.09848 8.00001L0.393839 13.7046C-0.131257 14.2297 -0.131257 15.0811 0.393839 15.6062C0.918935 16.1313 1.77028 16.1313 2.29538 15.6062L8.00002 9.90155L13.7046 15.6061C14.2297 16.1312 15.0811 16.1312 15.6062 15.6061C16.1313 15.081 16.1313 14.2297 15.6062 13.7046L9.90156 8.00001L15.6062 2.2954C16.1313 1.7703 16.1313 0.918956 15.6062 0.393861C15.0811 -0.131235 14.2297 -0.131235 13.7046 0.393861L8.00002 6.09847L2.29536 0.393822C1.77027 -0.131274 0.918919 -0.131274 0.393822 0.393822Z"
              fill="#151515"
            />
          </svg>
        </span>
      </div>
      <div className="error-body">
        <div className="error-body-text">
          <span className="error-body-text-h1">404</span>
          <span className="error-body-text-h2">Page not found!</span>
          <span className="error-body-text-p">
            We’re sorry, the page you requested could not be found. Please go
            back to the homepage!
          </span>
        </div>
        <button onClick={() => handlePage("home")} className="btn">
          GO HOME
        </button>
      </div>
    </div>
  );
}
