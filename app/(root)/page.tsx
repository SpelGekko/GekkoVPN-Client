"use client";

import React, { useState, useEffect } from "react";
import { useRouter } from "next/navigation";
import { invoke } from "@tauri-apps/api/core";

const LoginPage = () => {
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [message, setMessage] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const router = useRouter();

  useEffect(() => {
    const token = localStorage.getItem("vpn-token");
    if (token) {
      router.push("/dashboard");
    }
  }, [router]);

  const handleLogin = async (e: React.FormEvent) => {
    e.preventDefault();
    setIsLoading(true);
    setMessage("");

    try {
        const apiBaseUrl = process.env.NEXT_PUBLIC_BACKEND_API_BASE_URL;
        const apiKeyClient = process.env.NEXT_PUBLIC_API_KEY_CLIENT;

        if (!apiBaseUrl || !apiKeyClient) {
            setMessage("API configuration is missing.");
            console.error("Missing API configuration");
            return;
        }

        const response = await fetch(`${apiBaseUrl}/api/users/login`, {
            method: "POST",
            headers: {
                "Content-Type": "application/json",
                "X-API-KEY": apiKeyClient,
            },
            body: JSON.stringify({ email, password }),
        });

        const data = await response.json();

        if (!response.ok) {
            setMessage(data.message || "Login failed");
            return;
        }

        if (!data.user.isActive) {
            setMessage("Account is not verified. Please verify your email.");
            return;
        }

        // Save auth token
        localStorage.setItem("vpn-token", data.token);

        // Save VPN password with validation
        try {
          console.log("Saving VPN credentials...");
          const vpnPassword = password.trim();
          
          if (!vpnPassword) {
              throw new Error("Invalid password format");
          }
      
          // Save password in Rust backend with temporary storage
          await invoke("save_vpn_password", { 
              password: vpnPassword 
          });
      
          // Store username in localStorage for later association
          localStorage.setItem("vpn-username", data.user.username);
      
          console.log("VPN credentials saved successfully");
      } catch (error) {
          console.error("Failed to save VPN credentials:", error);
          setMessage("Warning: VPN credentials not saved properly");
      }

        // Log success and redirect
        console.log("Login successful - User:", {
            email: data.user.email,
            username: data.user.username,
            isActive: data.user.isActive
        });

        router.push("/dashboard");
    } catch (error) {
        console.error("Login error:", error);
        setMessage("Connection failed. Please try again.");
    } finally {
        setIsLoading(false);
    }
};

  return (
    <div className="flex items-center justify-center min-h-screen bg-gradient-to-b from-white via-emerald-50 to-white dark:from-gray-900 dark:via-gray-800 dark:to-gray-900">
      <div className="w-full max-w-md p-8 bg-white rounded-lg shadow-lg dark:bg-gray-800">
        <h1 className="text-2xl font-bold text-center text-gray-800 dark:text-white">
          Login to GekkoVPN
        </h1>
        <form onSubmit={handleLogin} className="mt-6">
          <div className="mb-4">
            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">
              Email:
            </label>
            <input
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              required
              disabled={isLoading}
              className="w-full px-4 py-2 mt-2 text-gray-700 bg-gray-100 border rounded-lg focus:outline-none focus:ring-2 focus:ring-emerald-500 dark:bg-gray-700 dark:text-gray-300 dark:border-gray-600"
            />
          </div>
          <div className="mb-4">
            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">
              Password:
            </label>
            <input
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              required
              disabled={isLoading}
              className="w-full px-4 py-2 mt-2 text-gray-700 bg-gray-100 border rounded-lg focus:outline-none focus:ring-2 focus:ring-emerald-500 dark:bg-gray-700 dark:text-gray-300 dark:border-gray-600"
            />
          </div>
          <button
            type="submit"
            disabled={isLoading}
            className={`w-full px-4 py-2 text-white ${
              isLoading 
                ? "bg-gray-400 cursor-not-allowed" 
                : "bg-emerald-500 hover:bg-emerald-600"
            } rounded-lg focus:outline-none focus:ring-2 focus:ring-emerald-500`}
          >
            {isLoading ? "Logging in..." : "Login"}
          </button>
        </form>
        {message && (
          <p className={`mt-4 text-center text-sm ${
            message.startsWith("Warning") 
              ? "text-yellow-500 dark:text-yellow-400"
              : "text-red-500 dark:text-red-400"
          }`}>
            {message}
          </p>
        )}
      </div>
    </div>
  );
};

export default LoginPage;