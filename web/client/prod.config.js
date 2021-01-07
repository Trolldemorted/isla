const Path = require('path');
const ExtractTextPlugin = require('extract-text-webpack-plugin')
const extractCSS = new ExtractTextPlugin({ filename: 'style.bundle.css' })

module.exports = {
  entry: './src/index.ts',
  mode: 'production',
  output: {
    filename: '[name].bundle.js',
    path: Path.resolve(__dirname, 'dist')
  },
  externals: [
    'fs'
  ],
  resolve: {
    extensions: [".ts", ".js"]
  },
  module: {
    rules: [{
        test: /\.tsx?$/,
        use: 'ts-loader',
        exclude: [/node_modules/, /tests/]
    },{
      test: /\.css$/,
      use: extractCSS.extract({
        fallback: 'style-loader',
        use: [ { loader: 'css-loader' } ]
      })
    }
   ]
  },
  plugins: [ extractCSS ]
};

